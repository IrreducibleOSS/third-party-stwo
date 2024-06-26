use std::collections::BTreeMap;

use self::accumulation::{DomainEvaluationAccumulator, PointEvaluationAccumulator};
use super::backend::Backend;
use super::channel::Blake2sChannel;
use super::circle::CirclePoint;
use super::fields::m31::BaseField;
use super::fields::qm31::SecureField;
use super::lookups::gkr_verifier::{Gate, GkrArtifact, GkrBatchProof};
use super::pcs::TreeVec;
use super::poly::circle::{CircleEvaluation, CirclePoly, SecureEvaluation};
use super::poly::BitReversedOrder;
use super::{ColumnVec, InteractionElements, LookupValues};

pub mod accumulation;
mod air_ext;
pub mod mask;

pub use air_ext::{AirExt, AirProverExt};
use num_traits::One;

/// Arithmetic Intermediate Representation (AIR).
/// An Air instance is assumed to already contain all the information needed to
/// evaluate the constraints.
/// For instance, all interaction elements are assumed to be present in it.
/// Therefore, an AIR is generated only after the initial trace commitment phase.
// TODO(spapini): consider renaming this struct.
pub trait Air {
    fn components(&self) -> Vec<&dyn Component>;
}

pub trait AirTraceVerifier {
    fn interaction_elements(&self, channel: &mut Blake2sChannel) -> InteractionElements;
}

pub trait AirTraceWriter<B: Backend>: AirTraceVerifier {
    fn interact(
        &self,
        channel: &mut Blake2sChannel,
        trace: &ColumnVec<CircleEvaluation<B, BaseField, BitReversedOrder>>,
        elements: &InteractionElements,
    ) -> (Vec<CirclePoly<B>>, GkrBatchProof, GkrArtifact, SecureField);

    fn to_air_prover(&self) -> &impl AirProver<B>;
}

pub trait AirProver<B: Backend>: Air {
    fn prover_components(&self) -> Vec<&dyn ComponentProver<B>>;
}

/// A component is a set of trace columns of various sizes along with a set of
/// constraints on them.
pub trait Component {
    fn n_constraints(&self) -> usize;

    fn max_constraint_log_degree_bound(&self) -> u32;

    /// Returns the number of interaction phases done by the component.
    fn n_interaction_phases(&self) -> u32;

    /// Returns the degree bounds of each trace column.
    fn trace_log_degree_bounds(&self) -> TreeVec<ColumnVec<u32>>;

    fn mask_points(
        &self,
        point: CirclePoint<SecureField>,
    ) -> TreeVec<ColumnVec<Vec<CirclePoint<SecureField>>>>;

    /// Returns the ids of the interaction elements used by the component.
    fn interaction_element_ids(&self) -> Vec<String>;

    /// Evaluates the constraint quotients combination of the component, given the mask values.
    fn evaluate_constraint_quotients_at_point(
        &self,
        point: CirclePoint<SecureField>,
        mask: &ColumnVec<Vec<SecureField>>,
        evaluation_accumulator: &mut PointEvaluationAccumulator,
        interaction_elements: &InteractionElements,
        lookup_values: &LookupValues,
    );

    // TODO: Return result.
    fn verify_succinct_multilinear_gkr_layer_claims(
        &self,
        point: &[SecureField],
        interaction_elements: &InteractionElements,
        gkr_claims_to_verify_by_instance: &[Vec<SecureField>],
    ) -> bool;

    // TODO: Docs. Mention something about how for lookups.
    fn eval_at_point_iop_claims_by_n_variables(
        &self,
        multilinear_eval_claims_by_instance: &[Vec<SecureField>],
    ) -> BTreeMap<u32, Vec<SecureField>>;

    fn gkr_lookup_instance_configs(&self) -> Vec<LookupInstanceConfig>;
}

pub struct LookupInstanceConfig {
    // TODO: Consider changing Gate to LookupType.
    pub variant: Gate,
    pub is_lookup_table: bool,
}

pub trait MultilinearPolynomial {
    fn eval(
        &self,
        interaction_elements: &InteractionElements,
        point: &[SecureField],
    ) -> SecureField;
}

pub trait TraceExprPolynomial {
    fn eval(
        &self,
        interaction_elements: &InteractionElements,
        mask: &ColumnVec<Vec<SecureField>>,
    ) -> SecureField;
}

/// Represents the polynomial
pub struct ConstantPolynomial(SecureField);

impl ConstantPolynomial {
    pub fn one() -> Self {
        Self(SecureField::one())
    }
}

impl MultilinearPolynomial for ConstantPolynomial {
    fn eval(
        &self,
        _interaction_elements: &InteractionElements,
        point: &[SecureField],
    ) -> SecureField {
        self.0
    }
}

pub enum ColumnEvaluator {
    Univariate(Box<dyn TraceExprPolynomial>),
    Multilinear(Box<dyn MultilinearPolynomial>),
}

pub enum LookupEvaluator {
    GrandProduct(ColumnEvaluator),
    LogUp {
        numerator: ColumnEvaluator,
        denominator: ColumnEvaluator,
    },
}

pub enum LookupVariant {
    /// Makes reference to values in a lookup table.
    Reference,
    /// Is a lookup table.
    Table,
}

pub struct LookupConfig {
    pub variant: LookupVariant,
    pub evaluator: LookupEvaluator,
}

impl LookupConfig {
    pub fn new(variant: LookupVariant, evaluator: LookupEvaluator) -> Self {
        Self { variant, evaluator }
    }
}

pub trait ComponentTraceWriter<B: Backend> {
    fn write_interaction_trace(
        &self,
        trace: &ColumnVec<&CircleEvaluation<B, BaseField, BitReversedOrder>>,
        elements: &InteractionElements,
    ) -> ColumnVec<SecureEvaluation<B>>;
}

pub trait ComponentProver<B: Backend>: Component {
    /// Evaluates the constraint quotients of the component on the evaluation domain.
    /// Accumulates quotients in `evaluation_accumulator`.
    fn evaluate_constraint_quotients_on_domain(
        &self,
        trace: &ComponentTrace<'_, B>,
        evaluation_accumulator: &mut DomainEvaluationAccumulator<B>,
        interaction_elements: &InteractionElements,
        lookup_values: &LookupValues,
    );

    fn lookup_values(&self, _trace: &ComponentTrace<'_, B>) -> LookupValues {
        LookupValues::default()
    }
}

/// A component trace is a set of polynomials for each column on that component.
/// Each polynomial is stored both in a coefficients, and evaluations form (for efficiency)
pub struct ComponentTrace<'a, B: Backend> {
    /// Polynomials for each column.
    pub polys: TreeVec<ColumnVec<&'a CirclePoly<B>>>,
    /// Evaluations for each column (evaluated on the commitment domains).
    pub evals: TreeVec<ColumnVec<&'a CircleEvaluation<B, BaseField, BitReversedOrder>>>,
}

impl<'a, B: Backend> ComponentTrace<'a, B> {
    pub fn new(
        polys: TreeVec<ColumnVec<&'a CirclePoly<B>>>,
        evals: TreeVec<ColumnVec<&'a CircleEvaluation<B, BaseField, BitReversedOrder>>>,
    ) -> Self {
        Self { polys, evals }
    }
}
