#[cfg(all(test, target_os = "macos"))]
mod metal_tests {
    use createxcrunch::{Config, CreateXVariant, RewardVariant, SaltVariant};
    
    #[test]
    fn test_metal_kernel_generation() {
        // Test kernel generation for different configurations
        let config = Config {
            gpu_device: 0,
            factory_address: [
                186, 94, 208, 153, 99, 61, 59, 49, 62, 77, 95, 123, 220, 19, 5, 211, 194, 139, 165, 237,
            ],
            salt_variant: SaltVariant::Random,
            create_variant: CreateXVariant::Create3,
            reward: RewardVariant::LeadingZeros { zeros_threshold: 1 },
            output: "test_output.txt",
            use_metal: true,
        };
        
        let kernel_src = createxcrunch::metal_kernel::mk_metal_kernel_src(&config);
        
        // Check that kernel contains expected macros
        assert!(kernel_src.contains("#define GENERATE_SEED() RANDOM()"));
        assert!(kernel_src.contains("#define SUCCESS_CONDITION() hasLeading(digest, 1)"));
        assert!(kernel_src.contains("#define CREATE3() RUN_CREATE3()"));
    }
    
    #[test]
    fn test_metal_kernel_with_pattern() {
        let config = Config {
            gpu_device: 0,
            factory_address: [
                186, 94, 208, 153, 99, 61, 59, 49, 62, 77, 95, 123, 220, 19, 5, 211, 194, 139, 165, 237,
            ],
            salt_variant: SaltVariant::Random,
            create_variant: CreateXVariant::Create2 { 
                init_code_hash: [0u8; 32] 
            },
            reward: RewardVariant::Matching { 
                pattern: "ba5edXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXba5ed".to_owned().into_boxed_str()
            },
            output: "test_output.txt",
            use_metal: true,
        };
        
        let kernel_src = createxcrunch::metal_kernel::mk_metal_kernel_src(&config);
        
        // Check pattern matching setup
        assert!(kernel_src.contains("constant char pattern[40]"));
        assert!(kernel_src.contains("#define SUCCESS_CONDITION() isMatching(&pattern[0], digest)"));
    }
    
    #[test]
    fn test_metal_kernel_sender_variant() {
        let calling_address = [52, 165, 10, 122, 39, 46, 134, 238, 48, 183, 167, 78, 54, 243, 240, 42, 241, 139, 30, 181];
        
        let config = Config {
            gpu_device: 0,
            factory_address: [
                186, 94, 208, 153, 99, 61, 59, 49, 62, 77, 95, 123, 220, 19, 5, 211, 194, 139, 165, 237,
            ],
            salt_variant: SaltVariant::Sender { calling_address },
            create_variant: CreateXVariant::Create3,
            reward: RewardVariant::TotalZeros { zeros_threshold: 2 },
            output: "test_output.txt",
            use_metal: true,
        };
        
        let kernel_src = createxcrunch::metal_kernel::mk_metal_kernel_src(&config);
        
        // Check sender variant setup
        assert!(kernel_src.contains("#define GENERATE_SEED() SENDER()"));
        assert!(kernel_src.contains("#define S1_12 52u"));
        assert!(kernel_src.contains("#define S1_13 165u"));
        assert!(kernel_src.contains("#define SUCCESS_CONDITION() hasTotal(digest, 2)"));
    }
    
    #[test]
    fn test_metal_kernel_crosschain_variant() {
        let mut chain_id = [0u8; 32];
        chain_id[31] = 1;
        
        let config = Config {
            gpu_device: 0,
            factory_address: [
                186, 94, 208, 153, 99, 61, 59, 49, 62, 77, 95, 123, 220, 19, 5, 211, 194, 139, 165, 237,
            ],
            salt_variant: SaltVariant::Crosschain { chain_id },
            create_variant: CreateXVariant::Create2 { init_code_hash: [0u8; 32] },
            reward: RewardVariant::LeadingAndTotalZeros {
                leading_zeros_threshold: 1,
                total_zeros_threshold: 2,
            },
            output: "test_output.txt",
            use_metal: true,
        };
        
        let kernel_src = createxcrunch::metal_kernel::mk_metal_kernel_src(&config);
        
        // Check crosschain variant setup
        assert!(kernel_src.contains("#define GENERATE_SEED() XCHAIN()"));
        assert!(kernel_src.contains("#define S1_32 0u")); // chain_id starts at offset 32
        assert!(kernel_src.contains("#define S1_63 1u")); // last byte of chain_id
        assert!(kernel_src.contains("#define SUCCESS_CONDITION() (hasLeading(digest, 1) && hasTotal(digest, 2))"));
    }
}