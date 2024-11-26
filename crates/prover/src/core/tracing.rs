use cfg_if::cfg_if;
use std::env;
use tracing_subscriber::{
	layer::SubscriberExt,
	registry::LookupSpan,
	util::{SubscriberInitExt, TryInitError},
};

#[cfg(not(feature = "perfetto"))]
pub struct TracingGuard;

#[cfg(feature = "perfetto")]
pub type TracingGuard = tracing_profile::PerfettoGuard;

fn with_perfetto<S>(
	subscriber: S,
) -> (impl SubscriberExt + for<'lookup> LookupSpan<'lookup>, TracingGuard)
where
	S: SubscriberExt + for<'lookup> LookupSpan<'lookup>,
{
	cfg_if! {
		if #[cfg(feature = "perfetto")] {
			let (layer, guard) = tracing_profile::PerfettoLayer::new_from_env().expect("failed to initialize perfetto layer");
			(subscriber.with(layer), guard)
		} else {
			(subscriber, TracingGuard{})
		}
	}
}

pub fn init_tracing() -> Result<TracingGuard, TryInitError> {
	use tracing_profile::{CsvLayer, PrintTreeConfig, PrintTreeLayer};

	if let Ok(csv_path) = env::var("PROFILE_CSV_FILE") {
		let (layer, guard) = with_perfetto(
			tracing_subscriber::registry()
				.with(CsvLayer::new(csv_path))
				.with(tracing_subscriber::fmt::layer()),
		);
		layer.try_init()?;

		Ok(guard)
	} else {
		let (layer, guard) = with_perfetto(
			tracing_subscriber::registry().with(PrintTreeLayer::new(PrintTreeConfig {
				attention_above_percent: 25.0,
				relevant_above_percent: 2.5,
				hide_below_percent: 1.0,
				display_unaccounted: false,
				accumulate_events: true,
			})),
		);
		layer.try_init()?;

		Ok(guard)
	}
}

/// Trace multiplication event
macro_rules! trace_multiplication {
    ($name: ty) => {
        #[cfg(feature = "trace_multiplications")]
        {
            tracing::event!(name: "mul", tracing::Level::TRACE, {lhs = stringify!($name), rhs = stringify!($name)});
        }
    };
    ($lhs: ty, $rhs: ty) => {
        #[cfg(feature = "trace_multiplications")]
        {
            tracing::event!(name: "mul", tracing::Level::TRACE, {lhs = stringify!($lhs), rhs = stringify!($rhs)});
        }
    };
}

pub(crate) use trace_multiplication;