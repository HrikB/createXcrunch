use crate::{Config, CreateXVariant, RewardVariant, SaltVariant};
use std::fmt::Write as _;

static KERNEL_TEMPLATE: &str = include_str!("./kernels/metal/keccak256_template.metal");

/// Creates the Metal kernel source code by populating the template with the
/// values from the Config object.
pub fn mk_metal_kernel_src(config: &Config) -> String {
    let mut src = String::with_capacity(2048 + KERNEL_TEMPLATE.len());

    let (caller, chain_id) = match config.salt_variant {
        SaltVariant::CrosschainSender {
            chain_id,
            calling_address,
        } => {
            writeln!(src, "#define GENERATE_SEED() SENDER_XCHAIN()").unwrap();
            (calling_address, Some(chain_id))
        }
        SaltVariant::Crosschain { chain_id } => {
            writeln!(src, "#define GENERATE_SEED() XCHAIN()").unwrap();
            ([0u8; 20], Some(chain_id))
        }
        SaltVariant::Sender { calling_address } => {
            writeln!(src, "#define GENERATE_SEED() SENDER()").unwrap();
            (calling_address, None)
        }
        SaltVariant::Random => {
            writeln!(src, "#define GENERATE_SEED() RANDOM()").unwrap();
            ([0u8; 20], None)
        }
    };

    match &config.reward {
        RewardVariant::LeadingZeros { zeros_threshold } => {
            writeln!(src, "#define PATTERN() 0").unwrap();
            writeln!(src, "constant char pattern[1] = {{}};").unwrap();
            writeln!(src, "#define LEADING_ZEROES {zeros_threshold}").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() hasLeading(digest, {zeros_threshold})").unwrap();
        }
        RewardVariant::TotalZeros { zeros_threshold } => {
            writeln!(src, "#define PATTERN() 0").unwrap();
            writeln!(src, "constant char pattern[1] = {{}};").unwrap();
            writeln!(src, "#define LEADING_ZEROES 0").unwrap();
            writeln!(src, "#define TOTAL_ZEROES {zeros_threshold}").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() hasTotal(digest, {zeros_threshold})").unwrap();
        }
        RewardVariant::LeadingAndTotalZeros {
            leading_zeros_threshold,
            total_zeros_threshold,
        } => {
            writeln!(src, "#define PATTERN() 0").unwrap();
            writeln!(src, "constant char pattern[1] = {{}};").unwrap();
            writeln!(src, "#define LEADING_ZEROES {leading_zeros_threshold}").unwrap();
            writeln!(src, "#define TOTAL_ZEROES {total_zeros_threshold}").unwrap();
            writeln!(
                src,
                "#define SUCCESS_CONDITION() (hasLeading(digest, {leading_zeros_threshold}) && hasTotal(digest, {total_zeros_threshold}))"
            )
            .unwrap();
        }
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold,
            total_zeros_threshold,
        } => {
            writeln!(src, "#define PATTERN() 0").unwrap();
            writeln!(src, "constant char pattern[1] = {{}};").unwrap();
            writeln!(src, "#define LEADING_ZEROES {leading_zeros_threshold}").unwrap();
            writeln!(src, "#define TOTAL_ZEROES {total_zeros_threshold}").unwrap();
            writeln!(
                src,
                "#define SUCCESS_CONDITION() (hasLeading(digest, {leading_zeros_threshold}) || hasTotal(digest, {total_zeros_threshold}))"
            )
            .unwrap();
        }
        RewardVariant::Matching { pattern } => {
            writeln!(src, "#define LEADING_ZEROES 0").unwrap();
            writeln!(src, "constant char pattern[40] = \"{pattern}\";").unwrap();
            writeln!(src, "#define PATTERN() \"{pattern}\"").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() isMatching(&pattern[0], digest)").unwrap();
        }
    };

    let init_code_hash = match config.create_variant {
        CreateXVariant::Create2 { init_code_hash } => {
            writeln!(src, "#define CREATE3()").unwrap();
            init_code_hash
        }
        CreateXVariant::Create3 => {
            writeln!(src, "#define CREATE3() RUN_CREATE3()").unwrap();
            writeln!(src, "#define RUN_CREATE3() {{ \\").unwrap();
            writeln!(src, "  keccakf(spongeBuffer); \\").unwrap();
            writeln!(src, "  for (int i = 12; i < 32; ++i) \\").unwrap();
            writeln!(src, "    sponge[i - 10] = sponge[i]; \\").unwrap();
            writeln!(src, "  sponge[0] = 0xd6u; \\").unwrap();
            writeln!(src, "  sponge[1] = 0x94u; \\").unwrap();
            writeln!(src, "  sponge[22] = 0x01u; \\").unwrap();
            writeln!(src, "  sponge[23] = 0x01u; \\").unwrap();
            writeln!(src, "  for (int i = 24; i < 135; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            writeln!(src, "  sponge[135] = 0x80u; \\").unwrap();
            writeln!(src, "  for (int i = 136; i < 200; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            writeln!(src, "}}").unwrap();
            crate::PROXY_CHILD_CODEHASH
        }
    };

    // Define S1_* and S2_* constants
    let caller = caller.iter();
    let chain_id = chain_id
        .iter()
        .flatten()
        .enumerate()
        .map(|(i, x)| (i + 20, x));
    caller.enumerate().chain(chain_id).for_each(|(i, x)| {
        writeln!(src, "#define S1_{} {}u", i + 12, x).unwrap();
    });

    let factory = config.factory_address.iter();
    let hash = init_code_hash.iter();
    let hash = hash.enumerate().map(|(i, x)| (i + 52, x));

    for (i, x) in factory.enumerate().chain(hash) {
        writeln!(src, "#define S2_{} {}u", i + 1, x).unwrap();
    }

    // Define the GENERATE_SEED_TEMPLATE macros
    writeln!(src, "#define GENERATE_SEED_TEMPLATE() {{ \\").unwrap();
    match config.salt_variant {
        SaltVariant::CrosschainSender { .. } => {
            writeln!(src, "  for (int i = 0; i < 12; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            for i in 12..64 {
                writeln!(src, "  sponge[{}] = S1_{}; \\", i, i).unwrap();
            }
            for i in 64..84 {
                writeln!(src, "  sponge[{}] = S1_{}; \\", i, i - 52).unwrap();
            }
            writeln!(src, "  sponge[84] = 1u; \\").unwrap();
            writeln!(src, "  sponge[85] = d_message[0]; \\").unwrap();
            writeln!(src, "  sponge[86] = d_message[1]; \\").unwrap();
            writeln!(src, "  sponge[87] = d_message[2]; \\").unwrap();
            writeln!(src, "  sponge[88] = d_message[3]; \\").unwrap();
            writeln!(src, "  nonce.uint32_t[0] = tid; \\").unwrap();
            writeln!(src, "  nonce.uint32_t[1] = d_nonce[0]; \\").unwrap();
            writeln!(src, "  sponge[89] = nonce.uint8_t[0]; \\").unwrap();
            writeln!(src, "  sponge[90] = nonce.uint8_t[1]; \\").unwrap();
            writeln!(src, "  sponge[91] = nonce.uint8_t[2]; \\").unwrap();
            writeln!(src, "  sponge[92] = nonce.uint8_t[3]; \\").unwrap();
            writeln!(src, "  sponge[93] = nonce.uint8_t[4]; \\").unwrap();
            writeln!(src, "  sponge[94] = nonce.uint8_t[5]; \\").unwrap();
            writeln!(src, "  sponge[95] = nonce.uint8_t[6]; \\").unwrap();
            writeln!(src, "  sponge[96] = 0x01u; \\").unwrap();
            writeln!(src, "  for (int i = 97; i < 135; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            writeln!(src, "  sponge[135] = 0x80u; \\").unwrap();
            writeln!(src, "  for (int i = 136; i < 200; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            writeln!(src, "  keccakf(spongeBuffer); \\").unwrap();
        }
        SaltVariant::Crosschain { .. } => {
            for i in 0..32 {
                writeln!(src, "  sponge[{}] = S1_{}; \\", i, i + 32).unwrap();
            }
            writeln!(src, "  for (int i = 32; i < 52; i++) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            writeln!(src, "  sponge[52] = 1; \\").unwrap();
            writeln!(src, "  sponge[53] = d_message[0]; \\").unwrap();
            writeln!(src, "  sponge[54] = d_message[1]; \\").unwrap();
            writeln!(src, "  sponge[55] = d_message[2]; \\").unwrap();
            writeln!(src, "  sponge[56] = d_message[3]; \\").unwrap();
            writeln!(src, "  nonce.uint32_t[0] = tid; \\").unwrap();
            writeln!(src, "  nonce.uint32_t[1] = d_nonce[0]; \\").unwrap();
            writeln!(src, "  sponge[57] = nonce.uint8_t[0]; \\").unwrap();
            writeln!(src, "  sponge[58] = nonce.uint8_t[1]; \\").unwrap();
            writeln!(src, "  sponge[59] = nonce.uint8_t[2]; \\").unwrap();
            writeln!(src, "  sponge[60] = nonce.uint8_t[3]; \\").unwrap();
            writeln!(src, "  sponge[61] = nonce.uint8_t[4]; \\").unwrap();
            writeln!(src, "  sponge[62] = nonce.uint8_t[5]; \\").unwrap();
            writeln!(src, "  sponge[63] = nonce.uint8_t[6]; \\").unwrap();
            writeln!(src, "  sponge[64] = 0x01u; \\").unwrap();
            writeln!(src, "  for (int i = 65; i < 135; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            writeln!(src, "  sponge[135] = 0x80u; \\").unwrap();
            writeln!(src, "  for (int i = 136; i < 200; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            writeln!(src, "  keccakf(spongeBuffer); \\").unwrap();
        }
        SaltVariant::Sender { .. } => {
            writeln!(src, "  for (int i = 0; i < 12; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            for i in 12..32 {
                writeln!(src, "  sponge[{}] = S1_{}; \\", i, i).unwrap();
            }
            for i in 32..52 {
                writeln!(src, "  sponge[{}] = S1_{}; \\", i, i - 20).unwrap();
            }
            writeln!(src, "  sponge[52] = 0u; \\").unwrap();
            writeln!(src, "  sponge[53] = d_message[0]; \\").unwrap();
            writeln!(src, "  sponge[54] = d_message[1]; \\").unwrap();
            writeln!(src, "  sponge[55] = d_message[2]; \\").unwrap();
            writeln!(src, "  sponge[56] = d_message[3]; \\").unwrap();
            writeln!(src, "  nonce.uint32_t[0] = tid; \\").unwrap();
            writeln!(src, "  nonce.uint32_t[1] = d_nonce[0]; \\").unwrap();
            writeln!(src, "  sponge[57] = nonce.uint8_t[0]; \\").unwrap();
            writeln!(src, "  sponge[58] = nonce.uint8_t[1]; \\").unwrap();
            writeln!(src, "  sponge[59] = nonce.uint8_t[2]; \\").unwrap();
            writeln!(src, "  sponge[60] = nonce.uint8_t[3]; \\").unwrap();
            writeln!(src, "  sponge[61] = nonce.uint8_t[4]; \\").unwrap();
            writeln!(src, "  sponge[62] = nonce.uint8_t[5]; \\").unwrap();
            writeln!(src, "  sponge[63] = nonce.uint8_t[6]; \\").unwrap();
            writeln!(src, "  sponge[64] = 0x01u; \\").unwrap();
            writeln!(src, "  for (int i = 65; i < 135; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            writeln!(src, "  sponge[135] = 0x80u; \\").unwrap();
            writeln!(src, "  for (int i = 136; i < 200; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            writeln!(src, "  keccakf(spongeBuffer); \\").unwrap();
        }
        SaltVariant::Random => {
            writeln!(src, "  sponge[0] = d_message[0]; \\").unwrap();
            writeln!(src, "  sponge[1] = d_message[1]; \\").unwrap();
            writeln!(src, "  sponge[2] = d_message[2]; \\").unwrap();
            writeln!(src, "  sponge[3] = d_message[3]; \\").unwrap();
            writeln!(src, "  nonce.uint32_t[0] = tid; \\").unwrap();
            writeln!(src, "  nonce.uint32_t[1] = d_nonce[0]; \\").unwrap();
            writeln!(src, "  sponge[4] = nonce.uint8_t[0]; \\").unwrap();
            writeln!(src, "  sponge[5] = nonce.uint8_t[1]; \\").unwrap();
            writeln!(src, "  sponge[6] = nonce.uint8_t[2]; \\").unwrap();
            writeln!(src, "  sponge[7] = nonce.uint8_t[3]; \\").unwrap();
            writeln!(src, "  sponge[8] = nonce.uint8_t[4]; \\").unwrap();
            writeln!(src, "  sponge[9] = nonce.uint8_t[5]; \\").unwrap();
            writeln!(src, "  sponge[10] = nonce.uint8_t[6]; \\").unwrap();
            writeln!(src, "  for (int i = 11; i < 32; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            writeln!(src, "  sponge[32] = 0x01u; \\").unwrap();
            writeln!(src, "  for (int i = 33; i < 135; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            writeln!(src, "  sponge[135] = 0x80u; \\").unwrap();
            writeln!(src, "  for (int i = 136; i < 200; ++i) \\").unwrap();
            writeln!(src, "    sponge[i] = 0; \\").unwrap();
            writeln!(src, "  keccakf(spongeBuffer); \\").unwrap();
        }
    }
    writeln!(src, "}}").unwrap();

    src.push_str(KERNEL_TEMPLATE);

    src
}