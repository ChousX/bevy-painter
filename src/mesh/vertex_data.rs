//! Per-vertex material blending data.

/// Material blending data for a single vertex.
///
/// Supports up to 4 materials per vertex with blend weights.
/// This is the natural maximum for cubic voxels where a vertex
/// can touch at most 8 voxels (corners), but in practice surface nets
/// vertices typically touch 2-4 materials at boundaries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VertexMaterialData {
    /// Up to 4 material indices into the texture palette.
    /// Unused slots should be 0.
    pub ids: [u8; 4],

    /// Blend weights for each material.
    /// Should sum to 255 for correct blending.
    pub weights: [u8; 4],
}

impl VertexMaterialData {
    /// Create vertex data for a single material with full weight.
    ///
    /// # Example
    /// ```
    /// use bevy_painter::mesh::VertexMaterialData;
    ///
    /// let data = VertexMaterialData::single(5); // 100% material 5
    /// assert_eq!(data.ids, [5, 0, 0, 0]);
    /// assert_eq!(data.weights, [255, 0, 0, 0]);
    /// ```
    #[inline]
    pub const fn single(material_id: u8) -> Self {
        Self {
            ids: [material_id, 0, 0, 0],
            weights: [255, 0, 0, 0],
        }
    }

    /// Create vertex data blending two materials at 50/50.
    ///
    /// # Example
    /// ```
    /// use bevy_painter::mesh::VertexMaterialData;
    ///
    /// let data = VertexMaterialData::blend2_half(0, 1); // 50% each
    /// assert_eq!(data.weights[0] + data.weights[1], 255);
    /// ```
    #[inline]
    pub const fn blend2_half(id0: u8, id1: u8) -> Self {
        Self {
            ids: [id0, id1, 0, 0],
            weights: [128, 127, 0, 0],
        }
    }

    /// Create vertex data blending two materials with a ratio.
    ///
    /// # Arguments
    /// * `id0` - First material index
    /// * `id1` - Second material index  
    /// * `ratio` - Blend ratio from 0.0 (100% id0) to 1.0 (100% id1)
    ///
    /// # Example
    /// ```
    /// use bevy_painter::mesh::VertexMaterialData;
    ///
    /// let data = VertexMaterialData::blend2(0, 1, 0.75); // 25% mat0, 75% mat1
    /// ```
    #[inline]
    pub fn blend2(id0: u8, id1: u8, ratio: f32) -> Self {
        let ratio = ratio.clamp(0.0, 1.0);
        let w1 = (ratio * 255.0).round() as u8;
        let w0 = 255 - w1;

        Self {
            ids: [id0, id1, 0, 0],
            weights: [w0, w1, 0, 0],
        }
    }

    /// Create vertex data blending three materials with normalized weights.
    ///
    /// Weights are automatically normalized to sum to 255.
    ///
    /// # Arguments
    /// * `id0`, `id1`, `id2` - Material indices
    /// * `w0`, `w1`, `w2` - Relative weights (will be normalized)
    ///
    /// # Example
    /// ```
    /// use bevy_painter::mesh::VertexMaterialData;
    ///
    /// // Equal blend of three materials
    /// let data = VertexMaterialData::blend3(0, 1, 2, 1.0, 1.0, 1.0);
    /// ```
    pub fn blend3(id0: u8, id1: u8, id2: u8, w0: f32, w1: f32, w2: f32) -> Self {
        let sum = w0 + w1 + w2;
        if sum < 0.0001 {
            return Self::single(id0);
        }

        let scale = 255.0 / sum;
        let w0_u8 = (w0 * scale).round() as u8;
        let w1_u8 = (w1 * scale).round() as u8;
        // Last weight absorbs rounding error
        let w2_u8 = 255u8.saturating_sub(w0_u8).saturating_sub(w1_u8);

        Self {
            ids: [id0, id1, id2, 0],
            weights: [w0_u8, w1_u8, w2_u8, 0],
        }
    }

    /// Create vertex data blending four materials with normalized weights.
    ///
    /// Weights are automatically normalized to sum to 255.
    ///
    /// # Arguments
    /// * `ids` - Four material indices
    /// * `weights` - Four relative weights (will be normalized)
    ///
    /// # Example
    /// ```
    /// use bevy_painter::mesh::VertexMaterialData;
    ///
    /// let data = VertexMaterialData::blend4(
    ///     [0, 1, 2, 3],
    ///     [1.0, 2.0, 1.0, 0.5]
    /// );
    /// ```
    pub fn blend4(ids: [u8; 4], weights: [f32; 4]) -> Self {
        let sum: f32 = weights.iter().sum();
        if sum < 0.0001 {
            return Self::single(ids[0]);
        }

        let scale = 255.0 / sum;
        let mut result = [0u8; 4];
        let mut running = 0u8;

        for i in 0..3 {
            result[i] = (weights[i] * scale).round() as u8;
            running = running.saturating_add(result[i]);
        }
        // Last weight absorbs rounding error
        result[3] = 255u8.saturating_sub(running);

        Self {
            ids,
            weights: result,
        }
    }

    /// Create vertex data with explicit IDs and weights.
    ///
    /// # Panics (Debug Only)
    /// In debug builds, panics if weights don't sum to 255.
    /// In release builds, the shader will normalize the weights.
    ///
    /// # Example
    /// ```
    /// use bevy_painter::mesh::VertexMaterialData;
    ///
    /// let data = VertexMaterialData::raw([0, 1, 0, 0], [200, 55, 0, 0]);
    /// ```
    pub fn raw(ids: [u8; 4], weights: [u8; 4]) -> Self {
        #[cfg(debug_assertions)]
        {
            let sum: u16 = weights.iter().map(|&w| w as u16).sum();
            debug_assert_eq!(
                sum, 255,
                "Material weights must sum to 255, got {}. Use blend2/blend3/blend4 for auto-normalization.",
                sum
            );
        }

        Self { ids, weights }
    }

    /// Pack material IDs into a u32 for the vertex attribute.
    #[inline]
    pub const fn pack_ids(&self) -> u32 {
        (self.ids[0] as u32)
            | ((self.ids[1] as u32) << 8)
            | ((self.ids[2] as u32) << 16)
            | ((self.ids[3] as u32) << 24)
    }

    /// Pack weights into a u32 for the vertex attribute.
    #[inline]
    pub const fn pack_weights(&self) -> u32 {
        (self.weights[0] as u32)
            | ((self.weights[1] as u32) << 8)
            | ((self.weights[2] as u32) << 16)
            | ((self.weights[3] as u32) << 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single() {
        let data = VertexMaterialData::single(5);
        assert_eq!(data.ids, [5, 0, 0, 0]);
        assert_eq!(data.weights, [255, 0, 0, 0]);
    }

    #[test]
    fn test_blend2_half() {
        let data = VertexMaterialData::blend2_half(0, 1);
        assert_eq!(data.ids, [0, 1, 0, 0]);
        assert_eq!(data.weights[0] as u16 + data.weights[1] as u16, 255);
    }

    #[test]
    fn test_blend2() {
        let data = VertexMaterialData::blend2(0, 1, 0.0);
        assert_eq!(data.weights, [255, 0, 0, 0]);

        let data = VertexMaterialData::blend2(0, 1, 1.0);
        assert_eq!(data.weights, [0, 255, 0, 0]);

        let data = VertexMaterialData::blend2(0, 1, 0.5);
        assert_eq!(data.weights[0] as u16 + data.weights[1] as u16, 255);
    }

    #[test]
    fn test_blend3_sums_to_255() {
        let data = VertexMaterialData::blend3(0, 1, 2, 1.0, 1.0, 1.0);
        let sum: u16 = data.weights.iter().map(|&w| w as u16).sum();
        assert_eq!(sum, 255);
    }

    #[test]
    fn test_blend4_sums_to_255() {
        let data = VertexMaterialData::blend4([0, 1, 2, 3], [1.0, 2.0, 3.0, 4.0]);
        let sum: u16 = data.weights.iter().map(|&w| w as u16).sum();
        assert_eq!(sum, 255);
    }

    #[test]
    fn test_pack_ids() {
        let data = VertexMaterialData {
            ids: [1, 2, 3, 4],
            weights: [255, 0, 0, 0],
        };
        assert_eq!(data.pack_ids(), 1 | (2 << 8) | (3 << 16) | (4 << 24));
    }

    #[test]
    fn test_pack_weights() {
        let data = VertexMaterialData {
            ids: [0, 0, 0, 0],
            weights: [100, 50, 75, 30],
        };
        assert_eq!(
            data.pack_weights(),
            100 | (50 << 8) | (75 << 16) | (30 << 24)
        );
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Material weights must sum to 255")]
    fn test_raw_panics_on_bad_weights() {
        VertexMaterialData::raw([0, 0, 0, 0], [100, 100, 0, 0]); // Sum = 200
    }
}
