use tracing::info;

/// Quantization type configuration
#[derive(Debug, Clone)]
pub enum QuantizationType {
    None,
    Scalar,
    Product,
}

impl QuantizationType {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "scalar" => QuantizationType::Scalar,
            "product" => QuantizationType::Product,
            _ => QuantizationType::None,
        }
    }
}

/// Log quantization configuration
pub fn log_quantization_config(quantization_type: &QuantizationType) {
    match quantization_type {
        QuantizationType::None => {
            info!("Qdrant using full precision vectors (no quantization)");
        }
        QuantizationType::Scalar => {
            info!(
                "Qdrant using scalar quantization (INT8): {}",
                "~75% memory reduction, <1% accuracy loss"
            );
        }
        QuantizationType::Product => {
            info!(
                "Qdrant using product quantization: {}",
                "~96% memory reduction, 2-5% accuracy loss"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantization_type_parsing() {
        assert!(matches!(
            QuantizationType::from_str("none"),
            QuantizationType::None
        ));
        assert!(matches!(
            QuantizationType::from_str("scalar"),
            QuantizationType::Scalar
        ));
        assert!(matches!(
            QuantizationType::from_str("product"),
            QuantizationType::Product
        ));
        assert!(matches!(
            QuantizationType::from_str("SCALAR"),
            QuantizationType::Scalar
        ));
        assert!(matches!(
            QuantizationType::from_str("invalid"),
            QuantizationType::None
        ));
    }
}
