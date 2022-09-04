use super::table::Table;
use crate::processor_table::ProcessorTable;
use algebra::Multivariate;
use algebra::PrimeFelt;

pub struct MemoryTable<E> {
    table: Table<E>,
}

impl<E: PrimeFelt> MemoryTable<E> {
    const CYCLE: usize = 0;
    const MP: usize = 1;
    const MEM_VAL: usize = 2;
    const DUMMY: usize = 3;
    // extension columns
    const PERMUTATION: usize = 4;

    /// Outputs an unpadded but interweaved matrix
    pub fn derive_matrix(processor_matrix: &[[E; 7]]) -> Vec<[E; 4]> {
        // copy unpadded rows and sort
        // TODO: sorted by IP and then CYCLE. Check to see if processor table sorts by
        // cycle.
        let mut matrix = processor_matrix
            .iter()
            .filter_map(|row| {
                if row[ProcessorTable::<E>::CURR_INSTR].is_zero() {
                    None
                } else {
                    Some([
                        row[ProcessorTable::<E>::CYCLE],
                        row[ProcessorTable::<E>::MP],
                        row[ProcessorTable::<E>::MEM_VAL],
                        E::zero(), // dummy=no
                    ])
                }
            })
            .collect::<Vec<[E; 4]>>();
        matrix.sort_by_key(|row| row[Self::MP].into_bigint());

        // insert dummy rows for smooth clk jumps
        for i in 0..matrix.len() - 1 {
            let curr_row = &matrix[i];
            let next_row = &matrix[i + 1];
            if curr_row[Self::MP] == next_row[Self::MP]
                && curr_row[Self::CYCLE] + E::one() != next_row[Self::CYCLE]
            {
                matrix.insert(
                    i + 1,
                    [
                        curr_row[Self::CYCLE] + E::one(),
                        curr_row[Self::MP],
                        curr_row[Self::MEM_VAL],
                        E::one(), // dummy=yes
                    ],
                )
            }
        }

        todo!()
    }

    pub fn pad(&mut self) {
        while !self.table.matrix.len().is_power_of_two() {
            let last_row = self.table.matrix.last().unwrap();
            self.table.matrix.push(vec![
                last_row[Self::CYCLE] + E::one(),
                last_row[Self::MP],
                last_row[Self::MEM_VAL],
                E::one(), // dummy=yes
            ]);
        }
    }

    fn transition_constraints(
        cycle: &Multivariate<E>,
        mp: &Multivariate<E>,
        mem_val: &Multivariate<E>,
        dummy: &Multivariate<E>,
        cycle_next: &Multivariate<E>,
        mp_next: &Multivariate<E>,
        mem_val_next: &Multivariate<E>,
        dummy_next: &Multivariate<E>,
    ) -> Vec<Multivariate<E>> {
        let one = E::one();
        vec![
            // 1. memory pointer increases by one or zero
            // note: remember table is sorted by memory address
            (mp_next.clone() - mp.clone() - one) * (mp_next.clone() - mp.clone()),
            //
            // 2. the memory value changes only if (a.) the memory pointer does not increase or
            // (b.) the cycle count increases by one. These constraints are implied by 3.
            //
            // 3. if the memory pointer increases by one, then the memory value must be set to zero
            (mp_next.clone() - mp.clone()) * mem_val_next.clone(),
            // 4. dummy has to be zero or one
            (dummy_next.clone() - one) * dummy_next.clone(),
            // 5. if dummy is set the memory pointer can not change
            (mp_next.clone() - mp.clone()) * dummy.clone(),
            // 6. if dummy is set the memory value can not change
            (mem_val_next.clone() - mem_val.clone()) * dummy.clone(),
            // 7. if the memory pointer remains the same, then the cycle has to increase by one
            (mp_next.clone() - mp.clone() - one) * (cycle_next.clone() - cycle.clone() - one),
        ]
    }

    fn base_boundary_constraints() -> Vec<Multivariate<E>> {
        let variables = Multivariate::<E>::variables(5);
        vec![
            variables[Self::CYCLE].clone(),
            variables[Self::MP].clone(),
            variables[Self::MEM_VAL].clone(),
        ]
    }

    fn extension_boundary_constraints(challenges: &[E]) -> Vec<Multivariate<E>> {
        let variables = Multivariate::<E>::variables(5);
        vec![
            variables[Self::CYCLE].clone(),
            variables[Self::MP].clone(),
            variables[Self::MEM_VAL].clone(),
            // TODO: why is this not included?
            // variables[Self::PERMUTATION].clone() - E::one(),
        ]
    }

    pub fn base_transition_constraints() -> Vec<Multivariate<E>> {
        let variables = Multivariate::<E>::variables(8);
        let cycle = variables[Self::CYCLE].clone();
        let mp = variables[Self::MP].clone();
        let mem_val = variables[Self::MEM_VAL].clone();
        let dummy = variables[Self::DUMMY].clone();
        let cycle_next = variables[4 + Self::CYCLE].clone();
        let mp_next = variables[4 + Self::MP].clone();
        let mem_val_next = variables[4 + Self::MEM_VAL].clone();
        let dummy_next = variables[4 + Self::DUMMY].clone();
        Self::transition_constraints(
            &cycle,
            &mp,
            &mem_val,
            &dummy,
            &cycle_next,
            &mp_next,
            &mem_val_next,
            &dummy_next,
        )
    }

    fn extension_transition_constraints(challenges: &[E]) -> Vec<Multivariate<E>> {
        let mut challenges_iter = challenges.iter().copied();
        let a = challenges_iter.next().unwrap();
        let b = challenges_iter.next().unwrap();
        let c = challenges_iter.next().unwrap();
        let d = challenges_iter.next().unwrap();
        let e = challenges_iter.next().unwrap();
        let f = challenges_iter.next().unwrap();
        let alpha = challenges_iter.next().unwrap();
        let beta = challenges_iter.next().unwrap();
        let gamma = challenges_iter.next().unwrap();
        let delta = challenges_iter.next().unwrap();
        let eta = challenges_iter.next().unwrap();

        let variables = Multivariate::<E>::variables(10);
        let cycle = variables[Self::CYCLE].clone();
        let mp = variables[Self::MP].clone();
        let mem_val = variables[Self::MEM_VAL].clone();
        let dummy = variables[Self::DUMMY].clone();
        let permutation = variables[Self::PERMUTATION].clone();
        let cycle_next = variables[5 + Self::CYCLE].clone();
        let mp_next = variables[5 + Self::MP].clone();
        let mem_val_next = variables[5 + Self::MEM_VAL].clone();
        let dummy_next = variables[5 + Self::DUMMY].clone();
        let permutation_next = variables[5 + Self::PERMUTATION].clone();

        let mut polynomials = Self::transition_constraints(
            &cycle,
            &mp,
            &mem_val,
            &dummy,
            &cycle_next,
            &mp_next,
            &mem_val_next,
            &dummy_next,
        );

        let permutation_constraint = (permutation_next.clone()
            - permutation.clone()
                * (Multivariate::constant(beta)
                    - cycle.clone() * d
                    - mp.clone() * e
                    - mem_val.clone() * f))
            * (dummy.clone() - E::one())
            + (permutation_next.clone() - permutation.clone()) * dummy.clone();
        polynomials.push(permutation_constraint);

        polynomials
    }

    fn extension_terminal_constraints(challenges: &[E], terminals: &[E]) -> Vec<Multivariate<E>> {
        let mut challenges_iter = challenges.iter().copied();
        let a = challenges_iter.next().unwrap();
        let b = challenges_iter.next().unwrap();
        let c = challenges_iter.next().unwrap();
        let d = challenges_iter.next().unwrap();
        let e = challenges_iter.next().unwrap();
        let f = challenges_iter.next().unwrap();
        let alpha = challenges_iter.next().unwrap();
        let beta = challenges_iter.next().unwrap();
        let gamma = challenges_iter.next().unwrap();
        let delta = challenges_iter.next().unwrap();
        let eta = challenges_iter.next().unwrap();

        let mut terminal_iter = terminals.iter().copied();
        let processor_instruction_permutation_terminal = terminal_iter.next().unwrap();
        let processor_memory_permutation_terminal = terminal_iter.next().unwrap();
        let processor_input_evaluation_terminal = terminal_iter.next().unwrap();
        let processor_output_evaluation_terminal = terminal_iter.next().unwrap();
        let instruction_evaluation_terminal = terminal_iter.next().unwrap();

        let variables = Multivariate::<E>::variables(5);
        let cycle = variables[Self::CYCLE].clone();
        let mp = variables[Self::MP].clone();
        let mem_val = variables[Self::MEM_VAL].clone();
        let dummy = variables[Self::DUMMY].clone();
        let permutation = variables[Self::PERMUTATION].clone();

        vec![
            (permutation.clone()
                * (Multivariate::constant(beta)
                    - cycle.clone() * d
                    - mp.clone() * e
                    - mem_val.clone() * f)
                - processor_memory_permutation_terminal)
                * (dummy.clone() - E::one())
                + (permutation.clone() - processor_memory_permutation_terminal) * dummy.clone(),
        ]
    }
}