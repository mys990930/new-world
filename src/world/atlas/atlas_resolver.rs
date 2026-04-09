use super::atlas_fields::AtlasFieldMap;
use super::scale::{AtlasArea, AtlasCoord, AtlasGrid};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalClass {
    Polar,
    Cold,
    Temperate,
    Warm,
    Hot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoistureClass {
    Arid,
    SemiArid,
    Subhumid,
    Humid,
    Wet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainFormClass {
    Plain,
    Hill,
    Mountain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayClass {
    None,
    Ocean,
    Coast,
    Riverine,
    Wetland,
    Alpine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiomePreview {
    Ocean,
    Coast,
    PolarTundra,
    Alpine,
    Mountain,
    Wetland,
    Riverplain,
    Desert,
    Steppe,
    Grassland,
    TemperateForest,
    BorealForest,
    TropicalForest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasResolvedCell {
    pub thermal: ThermalClass,
    pub moisture: MoistureClass,
    pub form: TerrainFormClass,
    pub overlay: OverlayClass,
    pub biome: BiomePreview,
}

#[derive(Debug, Clone)]
pub struct AtlasResolvedMap {
    cells: AtlasGrid<AtlasResolvedCell>,
}

impl AtlasResolvedMap {
    pub fn cells(&self) -> &AtlasGrid<AtlasResolvedCell> {
        &self.cells
    }

    pub fn area(&self) -> AtlasArea {
        self.cells.area()
    }

    pub fn get(&self, coord: AtlasCoord) -> Option<&AtlasResolvedCell> {
        self.cells.get(coord)
    }
}

pub fn resolve_atlas(fields: &AtlasFieldMap) -> AtlasResolvedMap {
    let mut resolved = Vec::with_capacity(fields.cells().values().len());
    for cell in fields.cells().values() {
        let thermal = dominant_thermal(cell);
        let moisture = dominant_moisture(cell);
        let form = dominant_form(cell);
        let overlay = dominant_overlay(cell);
        let biome = classify_biome(cell, thermal, moisture, form, overlay);

        resolved.push(AtlasResolvedCell {
            thermal,
            moisture,
            form,
            overlay,
            biome,
        });
    }

    AtlasResolvedMap {
        cells: AtlasGrid::from_values(fields.area(), resolved),
    }
}

fn dominant_thermal(cell: &super::atlas_fields::AtlasCell) -> ThermalClass {
    let values = [
        (cell.thermal.polar, ThermalClass::Polar),
        (cell.thermal.cold, ThermalClass::Cold),
        (cell.thermal.temperate, ThermalClass::Temperate),
        (cell.thermal.warm, ThermalClass::Warm),
        (cell.thermal.hot, ThermalClass::Hot),
    ];
    dominant_by_weight(values, ThermalClass::Temperate)
}

fn dominant_moisture(cell: &super::atlas_fields::AtlasCell) -> MoistureClass {
    let values = [
        (cell.moisture.arid, MoistureClass::Arid),
        (cell.moisture.semi_arid, MoistureClass::SemiArid),
        (cell.moisture.subhumid, MoistureClass::Subhumid),
        (cell.moisture.humid, MoistureClass::Humid),
        (cell.moisture.wet, MoistureClass::Wet),
    ];
    dominant_by_weight(values, MoistureClass::Subhumid)
}

fn dominant_form(cell: &super::atlas_fields::AtlasCell) -> TerrainFormClass {
    let values = [
        (cell.form.plain, TerrainFormClass::Plain),
        (cell.form.hill, TerrainFormClass::Hill),
        (cell.form.mountain, TerrainFormClass::Mountain),
    ];
    dominant_by_weight(values, TerrainFormClass::Plain)
}

fn dominant_overlay(cell: &super::atlas_fields::AtlasCell) -> OverlayClass {
    let values = [
        (cell.overlay.ocean, OverlayClass::Ocean),
        (cell.overlay.coast, OverlayClass::Coast),
        (cell.overlay.riverine, OverlayClass::Riverine),
        (cell.overlay.wetland, OverlayClass::Wetland),
        (cell.overlay.alpine, OverlayClass::Alpine),
    ];
    let overlay = dominant_by_weight(values, OverlayClass::None);

    if overlay == OverlayClass::Ocean {
        return OverlayClass::Ocean;
    }

    let strength = match overlay {
        OverlayClass::None => 0.0,
        OverlayClass::Ocean => cell.overlay.ocean,
        OverlayClass::Coast => cell.overlay.coast,
        OverlayClass::Riverine => cell.overlay.riverine,
        OverlayClass::Wetland => cell.overlay.wetland,
        OverlayClass::Alpine => cell.overlay.alpine,
    };

    if strength < 0.34 {
        OverlayClass::None
    } else {
        overlay
    }
}

fn classify_biome(
    cell: &super::atlas_fields::AtlasCell,
    thermal: ThermalClass,
    moisture: MoistureClass,
    form: TerrainFormClass,
    overlay: OverlayClass,
) -> BiomePreview {
    if overlay == OverlayClass::Ocean || cell.landness < 0.5 {
        return BiomePreview::Ocean;
    }
    if matches!(overlay, OverlayClass::Alpine) || (form == TerrainFormClass::Mountain && cell.alpine_factor > 0.48) {
        return BiomePreview::Alpine;
    }
    if overlay == OverlayClass::Wetland {
        return BiomePreview::Wetland;
    }
    if overlay == OverlayClass::Coast && cell.form.mountain < 0.45 {
        return BiomePreview::Coast;
    }
    if overlay == OverlayClass::Riverine && cell.wetness > 0.40 {
        return BiomePreview::Riverplain;
    }
    if thermal == ThermalClass::Polar {
        return BiomePreview::PolarTundra;
    }
    if form == TerrainFormClass::Mountain {
        return BiomePreview::Mountain;
    }

    if matches!(moisture, MoistureClass::Arid) {
        if matches!(thermal, ThermalClass::Warm | ThermalClass::Hot) {
            return BiomePreview::Desert;
        }
        return BiomePreview::Steppe;
    }

    if cell.cover.forest_potential > 0.60 {
        if matches!(thermal, ThermalClass::Hot | ThermalClass::Warm)
            && matches!(moisture, MoistureClass::Humid | MoistureClass::Wet)
        {
            return BiomePreview::TropicalForest;
        }
        if matches!(thermal, ThermalClass::Cold | ThermalClass::Polar) {
            return BiomePreview::BorealForest;
        }
        return BiomePreview::TemperateForest;
    }

    if cell.cover.grass_potential > 0.52 {
        return BiomePreview::Grassland;
    }

    BiomePreview::Steppe
}

fn dominant_by_weight<T: Copy, const N: usize>(pairs: [(f32, T); N], fallback: T) -> T {
    let mut best = fallback;
    let mut best_weight = -1.0_f32;

    for (weight, value) in pairs {
        if weight > best_weight {
            best_weight = weight;
            best = value;
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{AtlasArea, AtlasCoord, WorldMeta, generate_atlas_fields};

    #[test]
    fn resolve_atlas_preserves_cell_count() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 2, 2).unwrap();
        let fields = generate_atlas_fields(&WorldMeta::new(5), area);
        let resolved = resolve_atlas(&fields);

        assert_eq!(resolved.cells().values().len(), area.len());
    }
}
