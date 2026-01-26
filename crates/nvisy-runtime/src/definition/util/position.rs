//! Position type for visual editor node placement.

use serde::{Deserialize, Serialize};

/// Position of a node in the visual editor.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Position {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

impl Position {
    /// Creates a new position.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}
