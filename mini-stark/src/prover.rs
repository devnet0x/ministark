use crate::channel::ProverChannel;
use crate::composer::ConstraintComposer;
use crate::composer::DeepPolyComposer;
use crate::fri::FriProver;
use crate::trace::Queries;
use crate::utils::Timer;
use crate::Air;
use crate::Proof;
use crate::ProofOptions;
use crate::Trace;
use ark_ff::Field;
use fast_poly::GpuField;
use sha2::Sha256;

/// Errors that can occur during the proving stage
#[derive(Debug)]
pub enum ProvingError {
    Fail,
    // TODO
}

pub trait Prover {
    type Fp: GpuField;
    type Air: Air<Fp = Self::Fp>;
    type Trace: Trace<Fp = Self::Fp>;

    fn new(options: ProofOptions) -> Self;

    fn get_pub_inputs(&self, trace: &Self::Trace) -> <Self::Air as Air>::PublicInputs;

    fn options(&self) -> ProofOptions;

    fn generate_proof(&self, trace: Self::Trace) -> Result<Proof<Self::Air>, ProvingError> {
        let _timer = Timer::new("proof generation");

        let options = self.options();
        let trace_info = trace.info();
        let pub_inputs = self.get_pub_inputs(&trace);
        let air = Self::Air::new(trace_info, pub_inputs, options);
        air.validate();
        let mut channel = ProverChannel::<Self::Air, Sha256>::new(&air);

        let trace_domain = air.trace_domain();
        let lde_domain = air.lde_domain();
        let base_trace_polys = trace.base_columns().interpolate(trace_domain);
        assert_eq!(Self::Trace::NUM_BASE_COLUMNS, base_trace_polys.num_cols());
        let base_trace_lde = base_trace_polys.evaluate(lde_domain);
        let base_trace_lde_tree = base_trace_lde.commit_to_rows();
        channel.commit_base_trace(base_trace_lde_tree.root());
        let challenges = air.get_challenges(&mut channel.public_coin);

        #[cfg(debug_assertions)]
        let mut execution_trace = trace.base_columns().clone();
        let mut execution_trace_lde = base_trace_lde;
        let mut execution_trace_polys = base_trace_polys;
        let mut extension_trace_tree = None;
        let mut num_extension_columns = 0;

        if let Some(extension_trace) = trace.build_extension_columns(&challenges) {
            num_extension_columns = extension_trace.num_cols();
            let extension_trace_polys = extension_trace.interpolate(trace_domain);
            let extension_trace_lde = extension_trace_polys.evaluate(lde_domain);
            let extension_trace_lde_tree = extension_trace_lde.commit_to_rows();
            channel.commit_extension_trace(extension_trace_lde_tree.root());

            #[cfg(debug_assertions)]
            execution_trace.append(extension_trace);
            execution_trace_lde.append(extension_trace_lde);
            execution_trace_polys.append(extension_trace_polys);
            extension_trace_tree = Some(extension_trace_lde_tree);
        }

        assert_eq!(Self::Trace::NUM_EXTENSION_COLUMNS, num_extension_columns);

        #[cfg(debug_assertions)]
        air.validate_constraints(&challenges, &execution_trace);

        let composition_coeffs = air.get_constraint_composition_coeffs(&mut channel.public_coin);
        let constraint_coposer = ConstraintComposer::new(&air, composition_coeffs);
        // TODO: move commitment here
        let (composition_trace_lde, composition_trace_polys, composition_trace_lde_tree) =
            constraint_coposer.build_commitment(&challenges, &execution_trace_lde);
        channel.commit_composition_trace(composition_trace_lde_tree.root());

        let g = trace_domain.group_gen;
        let z = channel.get_ood_point();
        let ood_execution_trace_evals = execution_trace_polys.evaluate_at(z);
        let ood_execution_trace_evals_next = execution_trace_polys.evaluate_at(z * g);
        channel.send_ood_trace_states(&ood_execution_trace_evals, &ood_execution_trace_evals_next);
        let z_n = z.pow([composition_trace_polys.num_cols() as u64]);
        let ood_composition_trace_evals = composition_trace_polys.evaluate_at(z_n);
        channel.send_ood_constraint_evaluations(&ood_composition_trace_evals);

        let deep_coeffs = air.get_deep_composition_coeffs(&mut channel.public_coin);
        let mut deep_poly_composer = DeepPolyComposer::new(&air, deep_coeffs, z);
        deep_poly_composer.add_execution_trace_polys(
            execution_trace_polys,
            ood_execution_trace_evals,
            ood_execution_trace_evals_next,
        );
        deep_poly_composer
            .add_composition_trace_polys(composition_trace_polys, ood_composition_trace_evals);
        let deep_composition_poly = deep_poly_composer.into_deep_poly();
        let deep_composition_lde = deep_composition_poly.into_evaluations(lde_domain);

        let mut fri_prover = FriProver::<Self::Fp, Sha256>::new(air.options().into_fri_options());
        fri_prover.build_layers(&mut channel, deep_composition_lde.try_into().unwrap());

        channel.grind_fri_commitments();

        let query_positions = channel.get_fri_query_positions();
        let fri_proof = fri_prover.into_proof(&query_positions);

        let queries = Queries::new(
            &execution_trace_lde,
            &composition_trace_lde,
            base_trace_lde_tree,
            extension_trace_tree,
            composition_trace_lde_tree,
            &query_positions,
        );

        Ok(channel.build_proof(queries, fri_proof))
    }
}
