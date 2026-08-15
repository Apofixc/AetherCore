// Паритет с test_optional_deps.py: топологический DAG-резолвер,
// мягкая деградация при отсутствующих опциональных зависимостях

use nms_core::plugin::dag::{toposort, DagError, DagNode};

fn node(id: &str, deps: &[&str], optional: &[&str]) -> DagNode {
    DagNode {
        id: id.to_string(),
        deps: deps.iter().map(|s| s.to_string()).collect(),
        optional_deps: optional.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn test_providers_load_before_consumers() {
    let nodes = vec![
        node("dashboard", &["net-scanner"], &[]),
        node("net-scanner", &[], &[]),
    ];
    let result = toposort(&nodes).unwrap();
    let scanner_pos = result
        .order
        .iter()
        .position(|m| m == "net-scanner")
        .unwrap();
    let dash_pos = result.order.iter().position(|m| m == "dashboard").unwrap();
    assert!(scanner_pos < dash_pos);
}

#[test]
fn test_missing_required_dependency_fails() {
    let nodes = vec![node("dashboard", &["net-scanner"], &[])];
    assert!(matches!(
        toposort(&nodes).unwrap_err(),
        DagError::MissingDependency { .. }
    ));
}

#[test]
fn test_missing_optional_dependency_degrades_gracefully() {
    let nodes = vec![node("dashboard", &[], &["weather-widget"])];
    let result = toposort(&nodes).unwrap();
    assert_eq!(result.order, vec!["dashboard"]);
    assert_eq!(
        result.missing_optional.get("dashboard").unwrap(),
        &vec!["weather-widget".to_string()]
    );
}

#[test]
fn test_present_optional_dependency_orders_load() {
    let nodes = vec![
        node("dashboard", &[], &["weather-widget"]),
        node("weather-widget", &[], &[]),
    ];
    let result = toposort(&nodes).unwrap();
    let widget_pos = result
        .order
        .iter()
        .position(|m| m == "weather-widget")
        .unwrap();
    let dash_pos = result.order.iter().position(|m| m == "dashboard").unwrap();
    assert!(widget_pos < dash_pos);
    assert!(result.missing_optional.is_empty());
}

#[test]
fn test_cycle_detected() {
    let nodes = vec![node("a", &["b"], &[]), node("b", &["a"], &[])];
    assert!(matches!(
        toposort(&nodes).unwrap_err(),
        DagError::CyclicDependency(_)
    ));
}

#[test]
fn test_deterministic_order() {
    let nodes = vec![
        node("c", &[], &[]),
        node("a", &[], &[]),
        node("b", &[], &[]),
    ];
    let first = toposort(&nodes).unwrap().order;
    for _ in 0..10 {
        assert_eq!(toposort(&nodes).unwrap().order, first);
    }
}
