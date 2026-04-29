use super::graph::VoronoiSiteId;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ContinuousFieldSample {
    pub temperature: f32,
    pub hydration: f32,
    pub elevation: f32,
    pub continentality: f32,
    pub ruggedness: f32,
    pub oceanness: f32,
    pub mountainness: f32,
}

impl ContinuousFieldSample {
    pub fn clamped(self) -> Self {
        Self {
            temperature: clamp_unit_field(self.temperature),
            hydration: clamp_unit_field(self.hydration),
            elevation: self.elevation,
            continentality: clamp_unit_field(self.continentality),
            ruggedness: clamp_unit_field(self.ruggedness),
            oceanness: clamp_unit_field(self.oceanness),
            mountainness: clamp_unit_field(self.mountainness),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphInfluence {
    pub site: VoronoiSiteId,
    pub weight: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct VoronoiBlendSample {
    pub dominant_site: Option<VoronoiSiteId>,
    pub influences: Vec<GraphInfluence>,
    pub fields: ContinuousFieldSample,
    pub distance_to_boundary_blocks: f32,
}

pub fn clamp_unit_field(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

pub fn normalize_influences(influences: &mut [GraphInfluence]) {
    if influences.is_empty() {
        return;
    }

    let sum: f32 = influences
        .iter()
        .map(|influence| influence.weight.max(0.0))
        .sum();

    if sum <= f32::EPSILON {
        let uniform = 1.0 / influences.len() as f32;
        for influence in influences {
            influence.weight = uniform;
        }
        return;
    }

    for influence in influences {
        influence.weight = influence.weight.max(0.0) / sum;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_influences_clamps_negative_weights() {
        let mut influences = [
            GraphInfluence {
                site: VoronoiSiteId(1),
                weight: -1.0,
            },
            GraphInfluence {
                site: VoronoiSiteId(2),
                weight: 3.0,
            },
        ];

        normalize_influences(&mut influences);

        assert_eq!(influences[0].weight, 0.0);
        assert_eq!(influences[1].weight, 1.0);
    }

    #[test]
    fn normalize_influences_falls_back_to_uniform_weights() {
        let mut influences = [
            GraphInfluence {
                site: VoronoiSiteId(1),
                weight: 0.0,
            },
            GraphInfluence {
                site: VoronoiSiteId(2),
                weight: 0.0,
            },
        ];

        normalize_influences(&mut influences);

        assert_eq!(influences[0].weight, 0.5);
        assert_eq!(influences[1].weight, 0.5);
    }
}
