pub fn calculate_utilization(total_debt: u128, total_supply: u128) -> f64 {
    if total_supply == 0 {
        0.0
    } else {
        (total_debt as f64 / total_supply as f64) * 100.0
    }
}

pub fn ray_to_percent(ray_value: u128) -> f64 {
    (ray_value as f64 / 1e27) * 100.0
}
