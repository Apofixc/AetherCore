// Топологический DAG-резолвер порядка инициализации модулей
// Спецификация: MIGRATION_RUST_WASM.md, разделы 1.2.В и 1.4.Б (Способ 3, deps DAG)

use std::collections::{HashMap, HashSet, VecDeque};

/// Ошибка построения графа зависимостей
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum DagError {
    #[error("missing required dependency '{dep}' of module '{module}'")]
    MissingDependency { module: String, dep: String },
    #[error("cyclic dependency detected involving modules: {0:?}")]
    CyclicDependency(Vec<String>),
}

/// Узел графа зависимостей: идентификатор модуля и его обязательные/опциональные deps
#[derive(Debug, Clone)]
pub struct DagNode {
    pub id: String,
    pub deps: Vec<String>,
    pub optional_deps: Vec<String>,
}

/// Результат топологической сортировки
#[derive(Debug, Clone, PartialEq)]
pub struct TopoResult {
    /// Порядок инициализации: провайдеры (уровень 0) раньше потребителей
    pub order: Vec<String>,
    /// Опциональные зависимости, отсутствующие в наборе (монтируются как Host Trampoline)
    pub missing_optional: HashMap<String, Vec<String>>,
}

/// Топологическая сортировка модулей по алгоритму Кана.
/// Обязательные deps должны присутствовать; отсутствующие optional_deps
/// фиксируются отдельно для монтирования атомарных хост-трамплинов Err(NotAvailable).
pub fn toposort(nodes: &[DagNode]) -> Result<TopoResult, DagError> {
    let ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

    // Проверка наличия всех обязательных зависимостей
    for node in nodes {
        for dep in &node.deps {
            if !ids.contains(dep.as_str()) {
                return Err(DagError::MissingDependency {
                    module: node.id.clone(),
                    dep: dep.clone(),
                });
            }
        }
    }

    // Сбор отсутствующих опциональных зависимостей (мягкая деградация)
    let mut missing_optional: HashMap<String, Vec<String>> = HashMap::new();
    for node in nodes {
        let missing: Vec<String> = node
            .optional_deps
            .iter()
            .filter(|d| !ids.contains(d.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            missing_optional.insert(node.id.clone(), missing);
        }
    }

    // Алгоритм Кана: считаем входящие степени (модуль зависит от dep => dep раньше)
    let mut in_degree: HashMap<&str, usize> = nodes.iter().map(|n| (n.id.as_str(), 0)).collect();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in nodes {
        // Присутствующие опциональные зависимости также учитываются в порядке загрузки
        let all_deps = node.deps.iter().chain(
            node.optional_deps
                .iter()
                .filter(|d| ids.contains(d.as_str())),
        );
        for dep in all_deps {
            *in_degree.get_mut(node.id.as_str()).unwrap() += 1;
            dependents
                .entry(dep.as_str())
                .or_default()
                .push(node.id.as_str());
        }
    }

    // Стабильный детерминированный порядок: очередь по возрастанию id
    let mut queue: VecDeque<&str> = {
        let mut roots: Vec<&str> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();
        roots.sort_unstable();
        roots.into()
    };

    let mut order: Vec<String> = Vec::with_capacity(nodes.len());
    while let Some(id) = queue.pop_front() {
        order.push(id.to_string());
        let mut ready: Vec<&str> = Vec::new();
        if let Some(children) = dependents.get(id) {
            for &child in children {
                let deg = in_degree.get_mut(child).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    ready.push(child);
                }
            }
        }
        ready.sort_unstable();
        for r in ready {
            queue.push_back(r);
        }
    }

    // Если обработаны не все узлы — в графе есть цикл
    if order.len() != nodes.len() {
        let in_cycle: Vec<String> = nodes
            .iter()
            .filter(|n| !order.contains(&n.id))
            .map(|n| n.id.clone())
            .collect();
        return Err(DagError::CyclicDependency(in_cycle));
    }

    Ok(TopoResult {
        order,
        missing_optional,
    })
}
