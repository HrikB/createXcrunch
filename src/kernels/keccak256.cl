/*
   Copyright 2018 Lip Wee Yeo Amano

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

    https://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
*/

/**
* Based on the following, with small tweaks and optimizations:
* https://github.com/lwYeo/SoliditySHA3Miner/blob/master/SoliditySHA3Miner/Miner/Kernels/OpenCL/sha3KingKernel.cl
*
* Originally modified for OpenCL processing by lwYeo
*
* Original implementer: David Leon Gil
*
* License: CC0, attribution kindly requested. Blame taken too, but not
* liability.
*/

/**
 * A generalized GPU kernel template used for mining Ethereum addresses deployed 
 * through the CreateX contract factory:
 * https://github.com/pcaversaccio/createx
 *
 * This kernel is modified from two implementations:
 * https://github.com/0age/create2crunch/blob/master/src/kernels/keccak256.cl
 * This implementation is from create2crunch, however, that keccak256 implementation
 * is optimized to the point that it only calculates the proper values for the 
 * last 20 bytes (a partial keccak256). This is not sufficient for CreateX which
 * performs multiple keccak256 hashes. However, a partial keccak256 is used for the
 * last ones.
 *
 * https://github.com/Vectorized/function-selector-miner/blob/b900660837f5fac66b5837fdaa5b3f93ff1b0ad4/cpp/main.cpp
 * This implementation provides a full keccak256 implementation (although for a
 * different purpose).
 *
 * h/t https://github.com/Vectorized
 */

/******** Keccak-f[1600] (for finding efficient Ethereum addresses) ********/

#define OPENCL_PLATFORM_UNKNOWN 0
#define OPENCL_PLATFORM_AMD   2

#ifndef PLATFORM
# define PLATFORM       OPENCL_PLATFORM_UNKNOWN
#endif

#if PLATFORM == OPENCL_PLATFORM_AMD
# pragma OPENCL EXTENSION   cl_amd_media_ops : enable
#endif

typedef union _nonce_t
{
  ulong   uint64_t;
  uint    uint32_t[2];
  uchar   uint8_t[8];
} nonce_t;

#if PLATFORM == OPENCL_PLATFORM_AMD
static inline ulong ROL(const ulong x, const uint s)
{
  uint2 output;
  uint2 x2 = as_uint2(x);

  output = (s > 32u) ? amd_bitalign((x2).yx, (x2).xy, 64u - s) : amd_bitalign((x2).xy, (x2).yx, 32u - s);
  return as_ulong(output);
}
#else
#define ROL(X, S) (((X) << S) | ((X) >> (64 - S)))
#endif


#define THETA_(M, N, O) t = b[M] ^ ROL(b[N], 1); \
a[O + 0] = a[O + 0] ^ t; a[O + 5] = a[O + 5] ^ t; a[O + 10] = a[O + 10] ^ t; \
a[O + 15] = a[O + 15] ^ t; a[O + 20] = a[O + 20] ^ t;

#define THETA() \
b[0] = a[0] ^ a[5] ^ a[10] ^ a[15] ^ a[20]; \
b[1] = a[1] ^ a[6] ^ a[11] ^ a[16] ^ a[21]; \
b[2] = a[2] ^ a[7] ^ a[12] ^ a[17] ^ a[22]; \
b[3] = a[3] ^ a[8] ^ a[13] ^ a[18] ^ a[23]; \
b[4] = a[4] ^ a[9] ^ a[14] ^ a[19] ^ a[24]; \
THETA_(4, 1, 0); THETA_(0, 2, 1); THETA_(1, 3, 2); THETA_(2, 4, 3); THETA_(3, 0, 4);

#define RHO_PI_(M, N) t = b[0]; b[0] = a[M]; a[M] = ROL(t, N);

#define RHO_PI() t = a[1]; b[0] = a[10]; a[10] = ROL(t, 1); \
RHO_PI_(7, 3); RHO_PI_(11, 6); RHO_PI_(17, 10); RHO_PI_(18, 15); RHO_PI_(3, 21); RHO_PI_(5, 28); \
RHO_PI_(16, 36); RHO_PI_(8, 45); RHO_PI_(21, 55); RHO_PI_(24, 2); RHO_PI_(4, 14); RHO_PI_(15, 27); \
RHO_PI_(23, 41); RHO_PI_(19, 56); RHO_PI_(13, 8); RHO_PI_(12, 25); RHO_PI_(2, 43); RHO_PI_(20, 62); \
RHO_PI_(14, 18); RHO_PI_(22, 39); RHO_PI_(9, 61); RHO_PI_(6, 20); RHO_PI_(1, 44);

#define CHI_(N) \
b[0] = a[N + 0]; b[1] = a[N + 1]; b[2] = a[N + 2]; b[3] = a[N + 3]; b[4] = a[N + 4]; \
a[N + 0] = b[0] ^ ((~b[1]) & b[2]); \
a[N + 1] = b[1] ^ ((~b[2]) & b[3]); \
a[N + 2] = b[2] ^ ((~b[3]) & b[4]); \
a[N + 3] = b[3] ^ ((~b[4]) & b[0]); \
a[N + 4] = b[4] ^ ((~b[0]) & b[1]);

#define CHI() CHI_(0); CHI_(5); CHI_(10); CHI_(15); CHI_(20);

#define IOTA(X) a[0] = a[0] ^ X;

#define ITER(X) THETA(); RHO_PI(); CHI(); IOTA(X);

#define ITERS() \
ITER(0x0000000000000001); ITER(0x0000000000008082); \
ITER(0x800000000000808a); ITER(0x8000000080008000); \
ITER(0x000000000000808b); ITER(0x0000000080000001); \
ITER(0x8000000080008081); ITER(0x8000000000008009); \
ITER(0x000000000000008a); ITER(0x0000000000000088); \
ITER(0x0000000080008009); ITER(0x000000008000000a); \
ITER(0x000000008000808b); ITER(0x800000000000008b); \
ITER(0x8000000000008089); ITER(0x8000000000008003); \
ITER(0x8000000000008002); ITER(0x8000000000000080); \
ITER(0x000000000000800a); ITER(0x800000008000000a); \
ITER(0x8000000080008081); ITER(0x8000000000008080); \
ITER(0x0000000080000001); ITER(0x8000000080008008);

static inline void keccakf(ulong *a)
{
  ulong b[5];
  ulong t;
  ITERS();
}

static inline void partial_keccakf(ulong *a)
{
  ulong b[5];
  ulong t;
  ITER(0x0000000000000001); ITER(0x0000000000008082); 
  ITER(0x800000000000808a); ITER(0x8000000080008000);
  ITER(0x000000000000808b); ITER(0x0000000080000001);
  ITER(0x8000000080008081); ITER(0x8000000000008009);
  ITER(0x000000000000008a); ITER(0x0000000000000088);
  ITER(0x0000000080008009); ITER(0x000000008000000a);
  ITER(0x000000008000808b); ITER(0x800000000000008b);
  ITER(0x8000000000008089); ITER(0x8000000000008003);
  ITER(0x8000000000008002); ITER(0x8000000000000080);
  ITER(0x000000000000800a); ITER(0x800000008000000a);
  ITER(0x8000000080008081); ITER(0x8000000000008080);
  ITER(0x0000000080000001);

  // iteration 24 (partial)
#define o ((uint *)(a))
  // Theta (partial)
  b[0] = a[0] ^ a[5] ^ a[10] ^ a[15] ^ a[20];
  b[1] = a[1] ^ a[6] ^ a[11] ^ a[16] ^ a[21];
  b[2] = a[2] ^ a[7] ^ a[12] ^ a[17] ^ a[22];
  b[3] = a[3] ^ a[8] ^ a[13] ^ a[18] ^ a[23];
  b[4] = a[4] ^ a[9] ^ a[14] ^ a[19] ^ a[24];

  a[0] ^= b[4] ^ ROL(b[1], 1u);
  a[6] ^= b[0] ^ ROL(b[2], 1u);
  a[12] ^= b[1] ^ ROL(b[3], 1u);
  a[18] ^= b[2] ^ ROL(b[4], 1u);
  a[24] ^= b[3] ^ ROL(b[0], 1u);

  // Rho Pi (partial)
  o[3] = (o[13] >> 20) | (o[12] << 12);
  a[2] = ROL(a[12], 43);
  a[3] = ROL(a[18], 21);
  a[4] = ROL(a[24], 14);

  // Chi (partial)
  o[3] ^= ((~o[5]) & o[7]);
  o[4] ^= ((~o[6]) & o[8]);
  o[5] ^= ((~o[7]) & o[9]);
  o[6] ^= ((~o[8]) & o[0]);
  o[7] ^= ((~o[9]) & o[1]);
#undef o
}

static inline bool isMatching(uchar const *d)
{
  __constant char* pattern = PATTERN();

    #pragma unroll
    for (uint i = 0; i < 20; ++i) {
        uchar byte = d[i];

        // Extract the high and low nibbles
        char highNibble = (byte >> 4) & 0x0F;
        char lowNibble = byte & 0x0F;

        // Convert nibbles to hexadecimal characters
        char highChar = (highNibble < 10) ? ('0' + highNibble) : ('a' + highNibble - 10);
        char lowChar = (lowNibble < 10) ? ('0' + lowNibble) : ('a' + lowNibble - 10);

        // Get the corresponding characters from the pattern
        char patternHighChar = pattern[2 * i];     // Even index
        char patternLowChar = pattern[2 * i + 1];  // Odd index

        // Compare high nibble
        if (patternHighChar != 'X' && patternHighChar != highChar)
            return false;

        // Compare low nibble
        if (patternLowChar != 'X' && patternLowChar != lowChar)
            return false;
    }
    return true;
}

#define hasTotal(d) ( \
  (!(d[0])) + (!(d[1])) + (!(d[2])) + (!(d[3])) + \
  (!(d[4])) + (!(d[5])) + (!(d[6])) + (!(d[7])) + \
  (!(d[8])) + (!(d[9])) + (!(d[10])) + (!(d[11])) + \
  (!(d[12])) + (!(d[13])) + (!(d[14])) + (!(d[15])) + \
  (!(d[16])) + (!(d[17])) + (!(d[18])) + (!(d[19])) \
>= TOTAL_ZEROES)

#if LEADING_ZEROES == 8
#define hasLeading(d) (!(((uint*)d)[0]) && !(((uint*)d)[1]))
#elif LEADING_ZEROES == 7
#define hasLeading(d) (!(((uint*)d)[0]) && !(((uint*)d)[1] & 0x00ffffffu))
#elif LEADING_ZEROES == 6
#define hasLeading(d) (!(((uint*)d)[0]) && !(((uint*)d)[1] & 0x0000ffffu))
#elif LEADING_ZEROES == 5
#define hasLeading(d) (!(((uint*)d)[0]) && !(((uint*)d)[1] & 0x000000ffu))
#elif LEADING_ZEROES == 4
#define hasLeading(d) (!(((uint*)d)[0]))
#elif LEADING_ZEROES == 3
#define hasLeading(d) (!(((uint*)d)[0] & 0x00ffffffu))
#elif LEADING_ZEROES == 2
#define hasLeading(d) (!(((uint*)d)[0] & 0x0000ffffu))
#elif LEADING_ZEROES == 1
#define hasLeading(d) (!(((uint*)d)[0] & 0x000000ffu))
#else
static inline bool hasLeading(uchar const *d)
{
#pragma unroll
  for (uint i = 0; i < LEADING_ZEROES; ++i) {
    if (d[i] != 0) return false;
  }
  return true;
}
#endif

// Debugging helper
#define PRINT() { \
 printf("\ninput: "); \
  for (int i = 0; i < 85; ++i) \
    printf("%02x", sponge[i]); \
  printf("\ninput full: "); \
  for (int i = 0; i < 200; ++i) \
    printf("%02x", sponge[i]); \
  keccakf(spongeBuffer); \
  printf("\noutput: "); \
  for (int i = 0; i < 32; ++i) \
    printf("%02x", sponge[i]); \
  printf("\n"); \
  for (int i = 0; i < 20; ++i) \
    printf("%02x", digest[i]); \
  printf("\n"); \
}

#define SENDER() { \
  for (int i = 0; i < 12; ++i) \
    sponge[i] = 0; \
  sponge[12] = S1_12; \
  sponge[13] = S1_13; \
  sponge[14] = S1_14; \
  sponge[15] = S1_15; \
  sponge[16] = S1_16; \
  sponge[17] = S1_17; \
  sponge[18] = S1_18; \
  sponge[19] = S1_19; \
  sponge[20] = S1_20; \
  sponge[21] = S1_21; \
  sponge[22] = S1_22; \
  sponge[23] = S1_23; \
  sponge[24] = S1_24; \
  sponge[25] = S1_25; \
  sponge[26] = S1_26; \
  sponge[27] = S1_27; \
  sponge[28] = S1_28; \
  sponge[29] = S1_29; \
  sponge[30] = S1_30; \
  sponge[31] = S1_31; \
  sponge[32] = S1_12; \
  sponge[33] = S1_13; \
  sponge[34] = S1_14; \
  sponge[35] = S1_15; \
  sponge[36] = S1_16; \
  sponge[37] = S1_17; \
  sponge[38] = S1_18; \
  sponge[39] = S1_19; \
  sponge[40] = S1_20; \
  sponge[41] = S1_21; \
  sponge[42] = S1_22; \
  sponge[43] = S1_23; \
  sponge[44] = S1_24; \
  sponge[45] = S1_25; \
  sponge[46] = S1_26; \
  sponge[47] = S1_27; \
  sponge[48] = S1_28; \
  sponge[49] = S1_29; \
  sponge[50] = S1_30; \
  sponge[51] = S1_31;  \
  sponge[52] = 0u; \
  sponge[53] = d_message[0]; \
  sponge[54] = d_message[1]; \
  sponge[55] = d_message[2]; \
  sponge[56] = d_message[3]; \
  nonce.uint32_t[0] = get_global_id(0); \
  nonce.uint32_t[1] = d_nonce[0]; \
  sponge[57] = nonce.uint8_t[0]; \
  sponge[58] = nonce.uint8_t[1]; \
  sponge[59] = nonce.uint8_t[2]; \
  sponge[60] = nonce.uint8_t[3]; \
  sponge[61] = nonce.uint8_t[4]; \
  sponge[62] = nonce.uint8_t[5]; \
  sponge[63] = nonce.uint8_t[6]; \
  sponge[64] = 0x01u; \
  for (int i = 65; i < 135; ++i) \
    sponge[i] = 0; \
  sponge[135] = 0x80u; \
  for (int i = 136; i < 200; ++i) \
    sponge[i] = 0; \
  keccakf(spongeBuffer); \
}

#define SENDER_XCHAIN() { \
  for (int i = 0; i < 12; ++i) \
    sponge[i] = 0; \
  sponge[12] = S1_12; \
  sponge[13] = S1_13; \
  sponge[14] = S1_14; \
  sponge[15] = S1_15; \
  sponge[16] = S1_16; \
  sponge[17] = S1_17; \
  sponge[18] = S1_18; \
  sponge[19] = S1_19; \
  sponge[20] = S1_20; \
  sponge[21] = S1_21; \
  sponge[22] = S1_22; \
  sponge[23] = S1_23; \
  sponge[24] = S1_24; \
  sponge[25] = S1_25; \
  sponge[26] = S1_26; \
  sponge[27] = S1_27; \
  sponge[28] = S1_28; \
  sponge[29] = S1_29; \
  sponge[30] = S1_30; \
  sponge[31] = S1_31; \
  sponge[32] = S1_32; \
  sponge[33] = S1_33; \
  sponge[34] = S1_34; \
  sponge[35] = S1_35; \
  sponge[36] = S1_36; \
  sponge[37] = S1_37; \
  sponge[38] = S1_38; \
  sponge[39] = S1_39; \
  sponge[40] = S1_40; \
  sponge[41] = S1_41; \
  sponge[42] = S1_42; \
  sponge[43] = S1_43; \
  sponge[44] = S1_44; \
  sponge[45] = S1_45; \
  sponge[46] = S1_46; \
  sponge[47] = S1_47; \
  sponge[48] = S1_48; \
  sponge[49] = S1_49; \
  sponge[50] = S1_50; \
  sponge[51] = S1_51; \
  sponge[52] = S1_52; \
  sponge[53] = S1_53; \
  sponge[54] = S1_54; \
  sponge[55] = S1_55; \
  sponge[56] = S1_56; \
  sponge[57] = S1_57; \
  sponge[58] = S1_58; \
  sponge[59] = S1_59; \
  sponge[60] = S1_60; \
  sponge[61] = S1_61; \
  sponge[62] = S1_62; \
  sponge[63] = S1_63; \
  sponge[64] = S1_12; \
  sponge[65] = S1_13; \
  sponge[66] = S1_14; \
  sponge[67] = S1_15; \
  sponge[68] = S1_16; \
  sponge[69] = S1_17; \
  sponge[70] = S1_18; \
  sponge[71] = S1_19; \
  sponge[72] = S1_20; \
  sponge[73] = S1_21; \
  sponge[74] = S1_22; \
  sponge[75] = S1_23; \
  sponge[76] = S1_24; \
  sponge[77] = S1_25; \
  sponge[78] = S1_26; \
  sponge[79] = S1_27; \
  sponge[80] = S1_28; \
  sponge[81] = S1_29; \
  sponge[82] = S1_30; \
  sponge[83] = S1_31;  \
  sponge[84] = 1u; \
  sponge[85] = d_message[0]; \
  sponge[86] = d_message[1]; \
  sponge[87] = d_message[2]; \
  sponge[88] = d_message[3]; \
  nonce.uint32_t[0] = get_global_id(0); \
  nonce.uint32_t[1] = d_nonce[0]; \
  sponge[89] = nonce.uint8_t[0]; \
  sponge[90] = nonce.uint8_t[1]; \
  sponge[91] = nonce.uint8_t[2]; \
  sponge[92] = nonce.uint8_t[3]; \
  sponge[93] = nonce.uint8_t[4]; \
  sponge[94] = nonce.uint8_t[5]; \
  sponge[95] = nonce.uint8_t[6]; \
  sponge[96] = 0x01u; \
  for (int i = 97; i < 135; ++i) \
    sponge[i] = 0; \
  sponge[135] = 0x80u; \
  for (int i = 136; i < 200; ++i) \
    sponge[i] = 0; \
  keccakf(spongeBuffer); \
}

#define XCHAIN() { \
  sponge[0] = S1_32; \
  sponge[1] = S1_33; \
  sponge[2] = S1_34; \
  sponge[3] = S1_35; \
  sponge[4] = S1_36; \
  sponge[5] = S1_37; \
  sponge[6] = S1_38; \
  sponge[7] = S1_39; \
  sponge[8] = S1_40; \
  sponge[9] = S1_41; \
  sponge[10] = S1_42; \
  sponge[11] = S1_43; \
  sponge[12] = S1_44; \
  sponge[13] = S1_45; \
  sponge[14] = S1_46; \
  sponge[15] = S1_47; \
  sponge[16] = S1_48; \
  sponge[17] = S1_49; \
  sponge[18] = S1_50; \
  sponge[19] = S1_51; \
  sponge[20] = S1_52; \
  sponge[21] = S1_53; \
  sponge[22] = S1_54; \
  sponge[23] = S1_55; \
  sponge[24] = S1_56; \
  sponge[25] = S1_57; \
  sponge[26] = S1_58; \
  sponge[27] = S1_59; \
  sponge[28] = S1_60; \
  sponge[29] = S1_61; \
  sponge[30] = S1_62; \
  sponge[31] = S1_63; \
  for (int i = 32; i < 52; i++) \
    sponge[i] = 0; \
  sponge[52] = 1; \
  sponge[53] = d_message[0]; \
  sponge[54] = d_message[1]; \
  sponge[55] = d_message[2]; \
  sponge[56] = d_message[3]; \
  nonce.uint32_t[0] = get_global_id(0); \
  nonce.uint32_t[1] = d_nonce[0]; \
  sponge[57] = nonce.uint8_t[0]; \
  sponge[58] = nonce.uint8_t[1]; \
  sponge[59] = nonce.uint8_t[2]; \
  sponge[60] = nonce.uint8_t[3]; \
  sponge[61] = nonce.uint8_t[4]; \
  sponge[62] = nonce.uint8_t[5]; \
  sponge[63] = nonce.uint8_t[6]; \
  sponge[64] = 0x01u; \
  for (int i = 65; i < 135; ++i) \
    sponge[i] = 0; \
  sponge[135] = 0x80u; \
  for (int i = 136; i < 200; ++i) \
    sponge[i] = 0; \
  keccakf(spongeBuffer); \
}

#define RANDOM() { \
  sponge[0] = d_message[0]; \
  sponge[1] = d_message[1]; \
  sponge[2] = d_message[2]; \
  sponge[3] = d_message[3]; \
  nonce.uint32_t[0] = get_global_id(0); \
  nonce.uint32_t[1] = d_nonce[0]; \
  sponge[4] = nonce.uint8_t[0]; \
  sponge[5] = nonce.uint8_t[1]; \
  sponge[6] = nonce.uint8_t[2]; \
  sponge[7] = nonce.uint8_t[3]; \
  sponge[8] = nonce.uint8_t[4]; \
  sponge[9] = nonce.uint8_t[5]; \
  sponge[10] = nonce.uint8_t[6]; \
  for (int i = 11; i < 32; ++i) \
    sponge[i] = 0; \
  sponge[32] = 0x01u; \
  for (int i = 33; i < 135; ++i) \
    sponge[i] = 0; \
  sponge[135] = 0x80u; \
  for (int i = 136; i < 200; ++i) \
    sponge[i] = 0; \
  keccakf(spongeBuffer); \
}

#define RUN_CREATE3() { \
  keccakf(spongeBuffer); \
  for (int i = 12; i < 32; ++i) \
    sponge[i - 10] = sponge[i]; \
  sponge[0] = 0xd6u; \
  sponge[1] = 0x94u; \
  sponge[22] = 0x01u; \
  sponge[23] = 0x01u; \
  for (int i = 24; i < 135; ++i) \
    sponge[i] = 0; \
  sponge[135] = 0x80u; \
  for (int i = 136; i < 200; ++i) \
    sponge[i] = 0; \
}

__kernel void hashMessage(
  __constant uchar const *d_message,
  __constant uint const *d_nonce,
  __global volatile ulong *restrict solutions
) {
  ulong spongeBuffer[25];

#define sponge ((uchar *) spongeBuffer)
#define digest (sponge + 12)

  nonce_t nonce;

  // Salt hash
  GENERATE_SEED()

  // Move resulting hash into the right spot for CREATE2 Hash
#pragma unroll
  for (int i = 31; i >= 0; --i)
    sponge[i + 21] = sponge[i];

  // Setup Create2 Hash
  // write the control character
  sponge[0] = 0xffu;

  sponge[1] = S2_1;
  sponge[2] = S2_2;
  sponge[3] = S2_3;
  sponge[4] = S2_4;
  sponge[5] = S2_5;
  sponge[6] = S2_6;
  sponge[7] = S2_7;
  sponge[8] = S2_8;
  sponge[9] = S2_9;
  sponge[10] = S2_10;
  sponge[11] = S2_11;
  sponge[12] = S2_12;
  sponge[13] = S2_13;
  sponge[14] = S2_14;
  sponge[15] = S2_15;
  sponge[16] = S2_16;
  sponge[17] = S2_17;
  sponge[18] = S2_18;
  sponge[19] = S2_19;
  sponge[20] = S2_20;
  sponge[53] = S2_53;
  sponge[54] = S2_54;
  sponge[55] = S2_55;
  sponge[56] = S2_56;
  sponge[57] = S2_57;
  sponge[58] = S2_58;
  sponge[59] = S2_59;
  sponge[60] = S2_60;
  sponge[61] = S2_61;
  sponge[62] = S2_62;
  sponge[63] = S2_63;
  sponge[64] = S2_64;
  sponge[65] = S2_65;
  sponge[66] = S2_66;
  sponge[67] = S2_67;
  sponge[68] = S2_68;
  sponge[69] = S2_69;
  sponge[70] = S2_70;
  sponge[71] = S2_71;
  sponge[72] = S2_72;
  sponge[73] = S2_73;
  sponge[74] = S2_74;
  sponge[75] = S2_75;
  sponge[76] = S2_76;
  sponge[77] = S2_77;
  sponge[78] = S2_78;
  sponge[79] = S2_79;
  sponge[80] = S2_80;
  sponge[81] = S2_81;
  sponge[82] = S2_82;
  sponge[83] = S2_83;
  sponge[84] = S2_84;

  sponge[85] = 0x01u;

  // fill padding
#pragma unroll
  for (int i = 86; i < 135; ++i)
    sponge[i] = 0;

  // end padding
  sponge[135] = 0x80u;

  // fill remaining sponge state with zeros
#pragma unroll
  for (int i = 136; i < 200; ++i)
    sponge[i] = 0;

  // If this is a Create3 operation, setup and perform an additional CREATE hash
  CREATE3()

  partial_keccakf(spongeBuffer);

  // determine if the address meets the constraints
  if (
    SUCCESS_CONDITION()
  ) {
    // To be honest, if we are using OpenCL, 
    // we just need to write one solution for all practical purposes,
    // since the chance of multiple solutions appearing
    // in a single workset is extremely low.
    solutions[0] = nonce.uint64_t;

    // Pass back output address through solutions buffer.
    ulong newUint64 = 0;
  #pragma unroll
    for (ulong i = 0; i < 8; i++) {
      ulong d = digest[i];
      newUint64 |= (d << ((7 - i) * 8));
    }
    solutions[1] = newUint64;

    newUint64 = 0;
  #pragma unroll
    for (ulong j = 0; j < 8; j++) {
        ulong d = digest[j + 8];
        newUint64 |= (d << ((7 - j) * 8));
    }
    solutions[2] = newUint64;

    newUint64 = 0;
  #pragma unroll
    for (ulong k = 0; k < 8; k++) {
        ulong d = digest[k + 16];
        newUint64 |= (d << ((7 - k) * 8));
    }
    solutions[3] = newUint64;
  }
}
