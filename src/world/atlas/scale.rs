use std::error::Error;
use std::fmt::{self, Display, Formatter};

pub const ATLAS_CELL_SIZE_M: u32 = 256;
pub const ATLAS_CELL_SIZE_IN_CHUNKS: u32 = 16;
pub const ATLAS_CELL_SIZE_IN_REGIONS: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtlasCoord {
    pub x: i32,
    pub z: i32,
}

impl AtlasCoord {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasArea {
    origin: AtlasCoord,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasAreaError;

impl Display for AtlasAreaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("atlas area width and height must both be greater than zero")
    }
}

impl Error for AtlasAreaError {}

impl AtlasArea {
    pub fn new(origin: AtlasCoord, width: u32, height: u32) -> Result<Self, AtlasAreaError> {
        if width == 0 || height == 0 {
            return Err(AtlasAreaError);
        }

        Ok(Self {
            origin,
            width,
            height,
        })
    }

    pub const fn origin(self) -> AtlasCoord {
        self.origin
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }

    pub fn len(self) -> usize {
        self.width as usize * self.height as usize
    }

    pub fn contains(self, coord: AtlasCoord) -> bool {
        coord.x >= self.origin.x
            && coord.x < self.origin.x + self.width as i32
            && coord.z >= self.origin.z
            && coord.z < self.origin.z + self.height as i32
    }

    pub fn index_of(self, coord: AtlasCoord) -> Option<usize> {
        if !self.contains(coord) {
            return None;
        }

        let local_x = (coord.x - self.origin.x) as usize;
        let local_z = (coord.z - self.origin.z) as usize;
        Some(local_z * self.width as usize + local_x)
    }

    pub fn coord_at(self, index: usize) -> Option<AtlasCoord> {
        if index >= self.len() {
            return None;
        }

        let x = index % self.width as usize;
        let z = index / self.width as usize;
        Some(AtlasCoord::new(
            self.origin.x + x as i32,
            self.origin.z + z as i32,
        ))
    }

    pub fn coords(self) -> AtlasCoords {
        AtlasCoords {
            area: self,
            next: 0,
        }
    }
}

pub struct AtlasCoords {
    area: AtlasArea,
    next: usize,
}

impl Iterator for AtlasCoords {
    type Item = AtlasCoord;

    fn next(&mut self) -> Option<Self::Item> {
        let coord = self.area.coord_at(self.next)?;
        self.next += 1;
        Some(coord)
    }
}

#[derive(Debug, Clone)]
pub struct AtlasGrid<T> {
    area: AtlasArea,
    values: Vec<T>,
}

impl<T: Clone> AtlasGrid<T> {
    pub fn filled(area: AtlasArea, value: T) -> Self {
        Self {
            area,
            values: vec![value; area.len()],
        }
    }
}

impl<T: Default + Clone> AtlasGrid<T> {
    pub fn defaulted(area: AtlasArea) -> Self {
        Self::filled(area, T::default())
    }
}

impl<T> AtlasGrid<T> {
    pub(crate) fn from_values(area: AtlasArea, values: Vec<T>) -> Self {
        debug_assert_eq!(values.len(), area.len());
        Self { area, values }
    }

    pub const fn area(&self) -> AtlasArea {
        self.area
    }

    pub fn values(&self) -> &[T] {
        &self.values
    }

    pub fn values_mut(&mut self) -> &mut [T] {
        &mut self.values
    }

    pub fn get(&self, coord: AtlasCoord) -> Option<&T> {
        self.area.index_of(coord).map(|index| &self.values[index])
    }

    pub fn get_mut(&mut self, coord: AtlasCoord) -> Option<&mut T> {
        self.area
            .index_of(coord)
            .map(|index| &mut self.values[index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_area_rejects_zero_extent() {
        assert!(AtlasArea::new(AtlasCoord::new(0, 0), 0, 16).is_err());
        assert!(AtlasArea::new(AtlasCoord::new(0, 0), 16, 0).is_err());
    }

    #[test]
    fn atlas_area_indexes_coords_deterministically() {
        let area = AtlasArea::new(AtlasCoord::new(-2, 3), 4, 3).unwrap();
        let coord = AtlasCoord::new(0, 4);
        let index = area.index_of(coord).unwrap();

        assert_eq!(index, 6);
        assert_eq!(area.coord_at(index), Some(coord));
    }
}
