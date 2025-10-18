/// Connection pool status information.
#[derive(Debug, Clone)]
pub struct PoolStatus {
    /// Maximum number of connections in the pool
    pub max_size: usize,
    /// Current number of connections in the pool
    pub size: usize,
    /// Number of available connections
    pub available: usize,
    /// Number of requests waiting for connections
    pub waiting: usize,
}

impl PoolStatus {
    /// Returns the utilization percentage of the pool (0.0 to 1.0).
    #[inline]
    pub fn utilization(&self) -> f64 {
        if self.max_size == 0 {
            0.0
        } else {
            (self.size - self.available) as f64 / self.max_size as f64
        }
    }

    /// Returns whether the pool is under pressure (high utilization or waiting requests).
    #[inline]
    pub fn is_under_pressure(&self) -> bool {
        self.waiting > 0 || self.utilization() > 0.8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_status_utilization() {
        let status = PoolStatus {
            max_size: 10,
            size: 8,
            available: 2,
            waiting: 0,
        };

        // (8 - 2) / 10 = 0.6
        assert_eq!(status.utilization(), 0.6);
    }

    #[test]
    fn test_pool_status_pressure() {
        let high_util = PoolStatus {
            max_size: 10,
            size: 10,
            available: 1,
            waiting: 0,
        };
        assert!(high_util.is_under_pressure());

        let waiting = PoolStatus {
            max_size: 10,
            size: 5,
            available: 3,
            waiting: 2,
        };
        assert!(waiting.is_under_pressure());

        let normal = PoolStatus {
            max_size: 10,
            size: 5,
            available: 5,
            waiting: 0,
        };
        assert!(!normal.is_under_pressure());
    }
}
