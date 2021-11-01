use std::convert::TryInto;
use std::fmt::Debug;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::field::extension_field::quadratic::QuadraticExtension;
use crate::field::extension_field::{Extendable, FieldExtension};
use crate::field::field_types::{RichField, WIDTH};
use crate::field::goldilocks_field::GoldilocksField;
use crate::gates::poseidon::PoseidonGate;
use crate::hash::gmimc::GMiMC;
use crate::hash::hash_types::HashOut;
use crate::hash::hashing::{compress, hash_n_to_hash, PlonkyPermutation, PoseidonPermutation};
use crate::hash::poseidon::Poseidon;
use crate::iop::challenger::Challenger;
use crate::iop::target::{BoolTarget, Target};
use crate::plonk::circuit_builder::CircuitBuilder;

// const WIDTH: usize = 12;

pub trait Hasher<F: RichField>: Sized {
    /// Size of `Hash` in bytes.
    const HASH_SIZE: usize;
    type Hash: From<Vec<u8>>
        + Into<Vec<u8>>
        + Into<Vec<F>>
        + Into<u64>
        + Copy
        + Clone
        + Debug
        + Eq
        + PartialEq
        + Send
        + Sync
        + Serialize
        + DeserializeOwned;

    fn hash(input: Vec<F>, pad: bool) -> Self::Hash;
    fn two_to_one(left: Self::Hash, right: Self::Hash) -> Self::Hash;
}

#[derive(Copy, Clone)]
pub struct PoseidonHash;
impl<F: RichField> Hasher<F> for PoseidonHash {
    const HASH_SIZE: usize = 4 * 8;
    type Hash = HashOut<F>;

    fn hash(input: Vec<F>, pad: bool) -> Self::Hash {
        hash_n_to_hash::<F, PoseidonPermutation>(input, pad)
    }

    fn two_to_one(left: Self::Hash, right: Self::Hash) -> Self::Hash {
        compress::<F, <Self as AlgebraicHasher<F>>::Permutation>(left, right)
    }
}

impl<F: RichField> AlgebraicHasher<F> for PoseidonHash {
    type Permutation = PoseidonPermutation;

    fn permute_swapped<const D: usize>(
        inputs: [Target; WIDTH],
        swap: BoolTarget,
        builder: &mut CircuitBuilder<F, D>,
    ) -> [Target; WIDTH]
    where
        F: Extendable<D>,
    {
        let gate_type = PoseidonGate::<F, D, WIDTH>::new();
        let gate = builder.add_gate(gate_type, vec![]);

        let swap_wire = PoseidonGate::<F, D, WIDTH>::WIRE_SWAP;
        let swap_wire = Target::wire(gate, swap_wire);
        builder.connect(swap.target, swap_wire);

        // Route input wires.
        for i in 0..WIDTH {
            let in_wire = PoseidonGate::<F, D, WIDTH>::wire_input(i);
            let in_wire = Target::wire(gate, in_wire);
            builder.connect(inputs[i], in_wire);
        }

        // Collect output wires.
        (0..WIDTH)
            .map(|i| Target::wire(gate, PoseidonGate::<F, D, WIDTH>::wire_output(i)))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }

    fn observe_hash(hash: Self::Hash, challenger: &mut Challenger<F, Self>) {
        challenger.observe_hash(&hash)
    }
}

pub trait AlgebraicHasher<F: RichField>: Hasher<F, Hash = HashOut<F>> {
    // TODO: Adding a `const WIDTH: usize` here yields a compiler error down the line.
    // Maybe try again in a while.
    type Permutation: PlonkyPermutation<F>;
    fn permute_swapped<const D: usize>(
        inputs: [Target; WIDTH],
        swap: BoolTarget,
        builder: &mut CircuitBuilder<F, D>,
    ) -> [Target; WIDTH]
    where
        F: Extendable<D>;
    fn observe_hash(hash: Self::Hash, challenger: &mut Challenger<F, Self>);
}

pub trait GenericConfig<const D: usize>:
    Debug + Clone + Sync + Sized + Send + Eq + PartialEq
{
    type F: RichField + Extendable<D, Extension = Self::FE>;
    type FE: FieldExtension<D, BaseField = Self::F>;
    type Hasher: Hasher<Self::F>;
    type InnerHasher: AlgebraicHasher<Self::F>;
}

pub trait AlgebraicConfig<const D: usize>:
    Debug + Clone + Sync + Sized + Send + Eq + PartialEq
{
    type F: RichField + Extendable<D, Extension = Self::FE>;
    type FE: FieldExtension<D, BaseField = Self::F>;
    type Hasher: AlgebraicHasher<Self::F>;
    type InnerHasher: AlgebraicHasher<Self::F>;
}

impl<A: AlgebraicConfig<D>, const D: usize> GenericConfig<D> for A {
    type F = <Self as AlgebraicConfig<D>>::F;
    type FE = <Self as AlgebraicConfig<D>>::FE;
    type Hasher = <Self as AlgebraicConfig<D>>::Hasher;
    type InnerHasher = <Self as AlgebraicConfig<D>>::InnerHasher;
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PoseidonGoldilocksConfig;
impl AlgebraicConfig<2> for PoseidonGoldilocksConfig {
    type F = GoldilocksField;
    type FE = QuadraticExtension<Self::F>;
    type Hasher = PoseidonHash;
    type InnerHasher = PoseidonHash;
}
