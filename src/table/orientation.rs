//! Orientation system for blade data structures
//!
//! Provides O(1) view transformations while keeping physical storage columnar.
//! Implements 10 orientations: 8 D4 symmetries + X (elementwise) + R (scalar reduce).
//!
//! Design principles:
//! - Physical storage is always columnar: cols[col][row]
//! - (o ...) is O(1): changes view flag, never rewrites data
//! - Semantics match blawk behavior exactly

/// Orientation specifies how logical (i,j) maps to physical (row,col)
///
/// Physical storage is ALWAYS: Vec<Vec<f64>> where outer=columns, inner=rows
/// Orientation just changes the interpretation for operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ori {
    /// D4 symmetry group of rectangle (8 orientations)
    ///
    /// - swap: transpose (swap row/col dimensions)
    /// - flip_i: reverse row index
    /// - flip_j: reverse column index
    D4 {
        swap: bool,
        flip_i: bool,
        flip_j: bool,
    },

    /// X = Elementwise mode (operations apply element-by-element)
    Each,

    /// R = Real/scalar reduction mode (operations reduce to scalar)
    Real,
}

/// Orientation class determines operator dispatch strategy
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriClass {
    /// Column-wise like H: vectors are columns (contiguous in memory)
    ColwiseLike,

    /// Row-wise like Z: vectors are rows (strided in memory, needs tiling)
    RowwiseLike,

    /// Elementwise: Each operation (no vector structure)
    Each,

    /// Scalar: Real reduction mode
    Real,
}

/// Which axis is the "vector" axis for scanning operations
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VecAxis {
    /// Scan along i (row) dimension
    AlongI,

    /// Scan along j (column) dimension
    AlongJ,
}

/// Reduce mode determines output shape for aggregations
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReduceMode {
    /// Reduce along rows → output has ncols elements (one per column)
    ByCols,

    /// Reduce along columns → output has nrows elements (one per row)
    ByRows,

    /// Reduce all → output is single scalar
    Scalar,
}

impl Ori {
    /// Map logical indices (i,j) to physical (row,col)
    ///
    /// Physical storage is cols[col][row], so returned (row, col).
    ///
    /// # Arguments
    /// - nr: physical number of rows
    /// - nc: physical number of columns
    /// - i: logical row index (0..logical_nrows)
    /// - j: logical column index (0..logical_ncols)
    ///
    /// # Returns
    /// (physical_row, physical_col) for indexing cols[physical_col][physical_row]
    #[inline]
    pub fn map_ij(self, nr: usize, nc: usize, i: usize, j: usize) -> (usize, usize) {
        match self {
            Ori::D4 {
                swap,
                flip_i,
                flip_j,
            } => {
                let (mut ii, mut jj) = if swap {
                    // Transpose: logical (i,j) → (j,i) before flips
                    (j, i)
                } else {
                    (i, j)
                };

                // Apply flips in the logical space after potential swap
                if flip_i {
                    ii = (nr - 1) - ii;
                }
                if flip_j {
                    jj = (nc - 1) - jj;
                }

                (ii, jj)
            }
            Ori::Each | Ori::Real => {
                // Mapping irrelevant for elementwise/scalar modes
                (i, j)
            }
        }
    }

    /// Get logical shape under this orientation
    ///
    /// If swap=true, dimensions are transposed.
    ///
    /// # Arguments
    /// - nr: physical number of rows
    /// - nc: physical number of columns
    ///
    /// # Returns
    /// (logical_nrows, logical_ncols)
    #[inline]
    pub fn logical_shape(self, nr: usize, nc: usize) -> (usize, usize) {
        match self {
            Ori::D4 { swap: true, .. } => (nc, nr),  // Transposed
            Ori::D4 { swap: false, .. } => (nr, nc), // Normal
            Ori::Each | Ori::Real => (nr, nc),
        }
    }

    /// Get orientation class for dispatch
    pub fn class(self) -> OriClass {
        match self {
            Ori::D4 { swap: false, .. } => OriClass::ColwiseLike,
            Ori::D4 { swap: true, .. } => OriClass::RowwiseLike,
            Ori::Each => OriClass::Each,
            Ori::Real => OriClass::Real,
        }
    }

    /// Get vector axis for window operations
    ///
    /// Returns None for Each/Real modes.
    pub fn vec_axis(self) -> Option<VecAxis> {
        match self {
            Ori::D4 { swap: false, .. } => Some(VecAxis::AlongI), // Columns are vectors
            Ori::D4 { swap: true, .. } => Some(VecAxis::AlongJ),  // Rows are vectors
            Ori::Each | Ori::Real => None,
        }
    }

    /// Get reduce mode for aggregation operations
    pub fn reduce_mode(self) -> ReduceMode {
        match self {
            Ori::Real => ReduceMode::Scalar,
            Ori::D4 { swap: false, .. } => ReduceMode::ByCols, // ColwiseLike
            Ori::D4 { swap: true, .. } => ReduceMode::ByRows,  // RowwiseLike
            Ori::Each => ReduceMode::ByCols,                   // Default to colwise
        }
    }

    /// Get canonical name for this orientation
    ///
    /// Returns the standard short name (H, Z, N, _N, etc.)
    /// For synonyms (e.g., S and Z have same D4 values), returns the canonical one.
    ///
    /// # Example
    /// ```
    /// use blawktrust::{ORI_H, ORI_Z, ORI_S, ORI_X, ORI_R};
    ///
    /// assert_eq!(ORI_H.canonical_name(), "H");
    /// assert_eq!(ORI_Z.canonical_name(), "Z");
    /// assert_eq!(ORI_S.canonical_name(), "Z");  // S is synonym for Z
    /// assert_eq!(ORI_X.canonical_name(), "X");
    /// assert_eq!(ORI_R.canonical_name(), "R");
    /// ```
    pub fn canonical_name(self) -> &'static str {
        match self {
            Ori::Each => "X",
            Ori::Real => "R",
            Ori::D4 {
                swap: false,
                flip_i: false,
                flip_j: false,
            } => "H",
            Ori::D4 {
                swap: false,
                flip_i: true,
                flip_j: false,
            } => "N",
            Ori::D4 {
                swap: false,
                flip_i: false,
                flip_j: true,
            } => "_N",
            Ori::D4 {
                swap: false,
                flip_i: true,
                flip_j: true,
            } => "_H",
            Ori::D4 {
                swap: true,
                flip_i: false,
                flip_j: false,
            } => "Z", // Canonical for S
            Ori::D4 {
                swap: true,
                flip_i: true,
                flip_j: false,
            } => "_Z",
            Ori::D4 {
                swap: true,
                flip_i: false,
                flip_j: true,
            } => "_S",
            Ori::D4 {
                swap: true,
                flip_i: true,
                flip_j: true,
            } => "??", // Unused 8th D4 orientation
        }
    }
}

/// Orientation specification with name and metadata
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OriSpec {
    /// Name token (H, Z, N, S, _H, _Z, _N, _S, X, R)
    pub name: &'static str,

    /// Compass string from blawk (for reference/testing)
    pub compass: &'static str,

    /// The actual orientation
    pub ori: Ori,

    /// Derived class
    pub class: OriClass,
}

/// Orientation registry - 10 canonical orientations
pub const ORI_SPECS: [OriSpec; 10] = [
    // ===== Column-Major (ColwiseLike) =====
    // H = "NSWE": Normal, columns contiguous
    OriSpec {
        name: "H",
        compass: "NSWE",
        ori: Ori::D4 {
            swap: false,
            flip_i: false,
            flip_j: false,
        },
        class: OriClass::ColwiseLike,
    },
    // N = "SNWE": Rows reversed (South→North)
    OriSpec {
        name: "N",
        compass: "SNWE",
        ori: Ori::D4 {
            swap: false,
            flip_i: true,
            flip_j: false,
        },
        class: OriClass::ColwiseLike,
    },
    // _N = "NSEW": Columns reversed
    OriSpec {
        name: "_N",
        compass: "NSEW",
        ori: Ori::D4 {
            swap: false,
            flip_i: false,
            flip_j: true,
        },
        class: OriClass::ColwiseLike,
    },
    // _H = "SNEW": Both reversed
    OriSpec {
        name: "_H",
        compass: "SNEW",
        ori: Ori::D4 {
            swap: false,
            flip_i: true,
            flip_j: true,
        },
        class: OriClass::ColwiseLike,
    },
    // ===== Row-Major (RowwiseLike) =====
    // Z = "WENS": Normal row-major
    OriSpec {
        name: "Z",
        compass: "WENS",
        ori: Ori::D4 {
            swap: true,
            flip_i: false,
            flip_j: false,
        },
        class: OriClass::RowwiseLike,
    },
    // S = "EWNS": Synonym for Z
    OriSpec {
        name: "S",
        compass: "EWNS",
        ori: Ori::D4 {
            swap: true,
            flip_i: false,
            flip_j: false,
        },
        class: OriClass::RowwiseLike,
    },
    // _Z = "EWSN": Rows reversed
    OriSpec {
        name: "_Z",
        compass: "EWSN",
        ori: Ori::D4 {
            swap: true,
            flip_i: true,
            flip_j: false,
        },
        class: OriClass::RowwiseLike,
    },
    // _S = "WESN": Columns reversed
    OriSpec {
        name: "_S",
        compass: "WESN",
        ori: Ori::D4 {
            swap: true,
            flip_i: false,
            flip_j: true,
        },
        class: OriClass::RowwiseLike,
    },
    // ===== Special Modes =====
    // X = Elementwise mode
    OriSpec {
        name: "X",
        compass: "X",
        ori: Ori::Each,
        class: OriClass::Each,
    },
    // R = Scalar reduce mode
    OriSpec {
        name: "R",
        compass: "R",
        ori: Ori::Real,
        class: OriClass::Real,
    },
];

/// Look up orientation by name
pub fn lookup_ori(name: &str) -> Option<&'static OriSpec> {
    ORI_SPECS.iter().find(|spec| spec.name == name)
}

/// Standard orientation IDs
pub const ORI_H: Ori = Ori::D4 {
    swap: false,
    flip_i: false,
    flip_j: false,
};
pub const ORI_Z: Ori = Ori::D4 {
    swap: true,
    flip_i: false,
    flip_j: false,
};
pub const ORI_X: Ori = Ori::Each;
pub const ORI_R: Ori = Ori::Real;

// Column-major family (ColwiseLike)
pub const ORI_N: Ori = Ori::D4 {
    swap: false,
    flip_i: true,
    flip_j: false,
};
pub const ORI__N: Ori = Ori::D4 {
    swap: false,
    flip_i: false,
    flip_j: true,
};
pub const ORI__H: Ori = Ori::D4 {
    swap: false,
    flip_i: true,
    flip_j: true,
};

// Row-major family (RowwiseLike)
pub const ORI_S: Ori = Ori::D4 {
    swap: true,
    flip_i: false,
    flip_j: false,
};
pub const ORI__Z: Ori = Ori::D4 {
    swap: true,
    flip_i: true,
    flip_j: false,
};
pub const ORI__S: Ori = Ori::D4 {
    swap: true,
    flip_i: false,
    flip_j: true,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ori_h_identity() {
        // H orientation with 3x4 table
        let ori = ORI_H;
        let (nr, nc) = (3, 4);

        // Logical shape should match physical
        assert_eq!(ori.logical_shape(nr, nc), (3, 4));

        // Identity mapping
        assert_eq!(ori.map_ij(nr, nc, 0, 0), (0, 0));
        assert_eq!(ori.map_ij(nr, nc, 1, 2), (1, 2));
        assert_eq!(ori.map_ij(nr, nc, 2, 3), (2, 3));

        // Class should be colwise
        assert_eq!(ori.class(), OriClass::ColwiseLike);
        assert_eq!(ori.reduce_mode(), ReduceMode::ByCols);
    }

    #[test]
    fn test_ori_z_transpose() {
        // Z orientation (row-major)
        let ori = ORI_Z;
        let (nr, nc) = (3, 4);

        // Logical shape should be transposed
        assert_eq!(ori.logical_shape(nr, nc), (4, 3));

        // Transposed mapping: logical (i,j) → physical (j,i)
        assert_eq!(ori.map_ij(nr, nc, 0, 0), (0, 0));
        assert_eq!(ori.map_ij(nr, nc, 1, 2), (2, 1)); // swap!
        assert_eq!(ori.map_ij(nr, nc, 3, 2), (2, 3));

        // Class should be rowwise
        assert_eq!(ori.class(), OriClass::RowwiseLike);
        assert_eq!(ori.reduce_mode(), ReduceMode::ByRows);
    }

    #[test]
    fn test_ori_n_flip_rows() {
        // N orientation (rows reversed)
        let ori = Ori::D4 {
            swap: false,
            flip_i: true,
            flip_j: false,
        };
        let (nr, nc) = (3, 4);

        // Logical shape unchanged
        assert_eq!(ori.logical_shape(nr, nc), (3, 4));

        // Row index flipped: i → (nr-1)-i
        assert_eq!(ori.map_ij(nr, nc, 0, 0), (2, 0)); // row 0 → row 2
        assert_eq!(ori.map_ij(nr, nc, 1, 2), (1, 2)); // row 1 → row 1 (middle)
        assert_eq!(ori.map_ij(nr, nc, 2, 3), (0, 3)); // row 2 → row 0
    }

    #[test]
    fn test_ori_lookup() {
        // Test registry lookup
        let h = lookup_ori("H").unwrap();
        assert_eq!(h.name, "H");
        assert_eq!(h.compass, "NSWE");
        assert_eq!(h.ori, ORI_H);

        let z = lookup_ori("Z").unwrap();
        assert_eq!(z.name, "Z");
        assert_eq!(z.compass, "WENS");
        assert_eq!(z.ori, ORI_Z);

        let x = lookup_ori("X").unwrap();
        assert_eq!(x.ori, ORI_X);

        // Invalid name
        assert!(lookup_ori("INVALID").is_none());
    }

    #[test]
    fn test_all_ten_orientations() {
        // Verify all 10 orientations are registered
        assert_eq!(ORI_SPECS.len(), 10);

        let names: Vec<&str> = ORI_SPECS.iter().map(|s| s.name).collect();
        assert_eq!(
            names,
            vec!["H", "N", "_N", "_H", "Z", "S", "_Z", "_S", "X", "R"]
        );

        // Verify 4 colwise + 4 rowwise + 2 special
        let colwise = ORI_SPECS
            .iter()
            .filter(|s| s.class == OriClass::ColwiseLike)
            .count();
        let rowwise = ORI_SPECS
            .iter()
            .filter(|s| s.class == OriClass::RowwiseLike)
            .count();
        assert_eq!(colwise, 4);
        assert_eq!(rowwise, 4);
    }

    #[test]
    fn test_reduce_modes() {
        // Colwise → ByCols
        assert_eq!(ORI_H.reduce_mode(), ReduceMode::ByCols);

        // Rowwise → ByRows
        assert_eq!(ORI_Z.reduce_mode(), ReduceMode::ByRows);

        // Real → Scalar
        assert_eq!(ORI_R.reduce_mode(), ReduceMode::Scalar);

        // Each → ByCols (default)
        assert_eq!(ORI_X.reduce_mode(), ReduceMode::ByCols);
    }

    #[test]
    fn test_3x4_table_all_indices() {
        // Test table with distinct values: a[i][j] = 10*i + j
        // Physical storage: cols[col][row]
        let (nr, nc) = (3, 4);

        // For H orientation (identity)
        let ori = ORI_H;
        for i in 0..nr {
            for j in 0..nc {
                let (phys_r, phys_c) = ori.map_ij(nr, nc, i, j);
                assert_eq!(phys_r, i);
                assert_eq!(phys_c, j);
            }
        }

        // For Z orientation (transposed)
        let ori = ORI_Z;
        let (log_nr, log_nc) = ori.logical_shape(nr, nc);
        assert_eq!((log_nr, log_nc), (4, 3)); // Swapped!

        for i in 0..log_nr {
            for j in 0..log_nc {
                let (phys_r, phys_c) = ori.map_ij(nr, nc, i, j);
                // Transposed: logical (i,j) → physical (j,i)
                assert_eq!(phys_r, j);
                assert_eq!(phys_c, i);
            }
        }
    }

    #[test]
    fn test_canonical_names() {
        // Column-major orientations
        assert_eq!(ORI_H.canonical_name(), "H");
        assert_eq!(ORI_N.canonical_name(), "N");
        assert_eq!(ORI__N.canonical_name(), "_N");
        assert_eq!(ORI__H.canonical_name(), "_H");

        // Row-major orientations
        assert_eq!(ORI_Z.canonical_name(), "Z");
        assert_eq!(ORI_S.canonical_name(), "Z"); // S is synonym for Z
        assert_eq!(ORI__Z.canonical_name(), "_Z");
        assert_eq!(ORI__S.canonical_name(), "_S");

        // Special modes
        assert_eq!(ORI_X.canonical_name(), "X");
        assert_eq!(ORI_R.canonical_name(), "R");
    }
}
