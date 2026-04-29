mod cover;
mod domain;
mod material;
mod resolve;
mod seasonal;

pub use cover::{CoverOverrideRule, CoverPhase, cover_override_rule, default_cover_override_rules};
#[allow(unused_imports)]
pub use domain::{
    MaterialDomainInput, MaterialDomainKind, MaterialDomainSample, MaterialDomainSupport,
    MaterialDomainTransitionReason, material_domain_kind_for_region, sample_material_domain,
};
pub use material::{
    MaterialPolicyDef, MaterialPolicyId, default_material_policies, material_policy_def,
};
pub use resolve::{
    ChunkSurfacePlan, SurfaceColumnPlan, SurfaceRuntimeContext, empty_chunk_surface_plan,
    resolve_chunk_surface_plan, resolve_chunk_surface_plan_with_runtime,
    resolve_material_policy_for_archetype,
};
pub use seasonal::{
    SeasonalBiomeStateDef, SeasonalBiomeStateId, SeasonalPhase, default_seasonal_biome_states,
    seasonal_biome_state_def,
};
