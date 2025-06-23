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
 * Metal Shading Language implementation of Keccak256 for CreateX mining
 * Ported from OpenCL implementation for Apple Silicon GPU acceleration
 */

#include <metal_stdlib>
using namespace metal;

/******** Keccak-f[1600] (for finding efficient Ethereum addresses) ********/

typedef union _nonce_t
{
  ulong   uint64_t;
  uint    uint32_t[2];
  uchar   uint8_t[8];
} nonce_t;

#define ROL(X, S) (((X) << S) | ((X) >> (64 - S)))

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

static inline void keccakf(thread ulong *a)
{
  ulong b[5];
  ulong t;
  ITERS();
}

static inline void partial_keccakf(thread ulong *a)
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
#define o ((thread uint *)(a))
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

static inline bool isMatching(constant char* pattern, thread uchar const *d)
{
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

#define hasTotal(d, total_zeros) ( \
  (!(d[0])) + (!(d[1])) + (!(d[2])) + (!(d[3])) + \
  (!(d[4])) + (!(d[5])) + (!(d[6])) + (!(d[7])) + \
  (!(d[8])) + (!(d[9])) + (!(d[10])) + (!(d[11])) + \
  (!(d[12])) + (!(d[13])) + (!(d[14])) + (!(d[15])) + \
  (!(d[16])) + (!(d[17])) + (!(d[18])) + (!(d[19])) \
>= total_zeros)

static inline bool hasLeading(thread uchar const *d, uint leading_zeros)
{
  for (uint i = 0; i < leading_zeros; ++i) {
    if (d[i] != 0) return false;
  }
  return true;
}

#define SENDER() GENERATE_SEED_TEMPLATE()
#define SENDER_XCHAIN() GENERATE_SEED_TEMPLATE()
#define XCHAIN() GENERATE_SEED_TEMPLATE()
#define RANDOM() GENERATE_SEED_TEMPLATE()

// #define CREATE3() CREATE3_TEMPLATE() // This is defined in generated code

kernel void hashMessage(
  constant uchar* d_message [[buffer(0)]],
  constant uint* d_nonce [[buffer(1)]],
  device ulong* solutions [[buffer(2)]],
  constant uchar* config [[buffer(3)]],
  uint tid [[thread_position_in_grid]]
) {
  ulong spongeBuffer[25];

#define sponge ((thread uchar *) spongeBuffer)
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
    // Store the solution
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
    for (ulong k = 0; k < 4; k++) {
        ulong d = digest[k + 16];
        newUint64 |= (d << ((7 - k) * 8));
    }
    solutions[3] = newUint64;
  }
}