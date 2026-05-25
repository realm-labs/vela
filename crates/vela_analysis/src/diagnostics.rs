mod candidates;
mod effects;
mod match_exhaustiveness;
mod match_patterns;
mod member;

pub use effects::effect_diagnostics;
pub use match_exhaustiveness::match_exhaustiveness_diagnostics;
pub use match_patterns::match_pattern_diagnostics;
pub use member::member_access_diagnostics;
