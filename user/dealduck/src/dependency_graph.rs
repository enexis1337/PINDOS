#![no_std]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

/// Directed acyclic graph for dependency resolution
pub struct DependencyGraph {
    /// All nodes in the graph
    nodes: BTreeMap<String, Node>,
    /// Topological sort cache
    sorted_cache: Option<Vec<String>>,
}

impl DependencyGraph {
    /// Create a new empty graph
    pub fn new() -> Self {
        DependencyGraph {
            nodes: BTreeMap::new(),
            sorted_cache: None,
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, name: String) {
        if !self.nodes.contains_key(&name) {
            self.nodes.insert(name.clone(), Node::new(name));
            self.sorted_cache = None;
        }
    }

    /// Add an edge (dependency) from -> to
    /// This means 'to' depends on 'from' (from must start first)
    pub fn add_edge(&mut self, from: String, to: String) {
        // Ensure both nodes exist
        self.add_node(from.clone());
        self.add_node(to.clone());
        
        // Add edge
        if let Some(node) = self.nodes.get_mut(&to) {
            node.dependencies.push(from.clone());
        }
        
        if let Some(node) = self.nodes.get_mut(&from) {
            node.dependents.push(to.clone());
        }
        
        self.sorted_cache = None;
    }

    /// Remove a node from the graph
    pub fn remove_node(&mut self, name: &str) {
        if let Some(node) = self.nodes.remove(name) {
            // Remove all edges to this node
            for dependent in &node.dependents {
                if let Some(dep_node) = self.nodes.get_mut(dependent) {
                    dep_node.dependencies.retain(|d| d != name);
                }
            }
            
            for dependency in &node.dependencies {
                if let Some(dep_node) = self.nodes.get_mut(dependency) {
                    dep_node.dependents.retain(|d| d != name);
                }
            }
            
            self.sorted_cache = None;
        }
    }

    /// Get all dependencies of a node (transitive)
    pub fn get_dependencies(&self, name: &str) -> Vec<String> {
        let mut result = Vec::new();
        self.get_dependencies_recursive(name, &mut result);
        result
    }

    /// Recursive helper for get_dependencies
    fn get_dependencies_recursive(&self, name: &str, visited: &mut Vec<String>) {
        if visited.contains(&name.to_string()) {
            return;
        }
        
        visited.push(name.to_string());
        
        if let Some(node) = self.nodes.get(name) {
            for dep in &node.dependencies {
                self.get_dependencies_recursive(dep, visited);
            }
        }
    }

    /// Get all dependents of a node (transitive)
    pub fn get_dependents(&self, name: &str) -> Vec<String> {
        let mut result = Vec::new();
        self.get_dependents_recursive(name, &mut result);
        result
    }

    /// Recursive helper for get_dependents
    fn get_dependents_recursive(&self, name: &str, visited: &mut Vec<String>) {
        if visited.contains(&name.to_string()) {
            return;
        }
        
        visited.push(name.to_string());
        
        if let Some(node) = self.nodes.get(name) {
            for dep in &node.dependents {
                self.get_dependents_recursive(dep, visited);
            }
        }
    }

    /// Check if there's a cycle in the graph
    pub fn has_cycle(&self) -> bool {
        // Use DFS to detect cycles
        let mut visited = Vec::new();
        let mut recursion_stack = Vec::new();
        
        for node_name in self.nodes.keys() {
            if !visited.contains(node_name) {
                if self.detect_cycle_recursive(node_name, &mut visited, &mut recursion_stack) {
                    return true;
                }
            }
        }
        
        false
    }

    /// Recursive cycle detection
    fn detect_cycle_recursive(
        &self,
        node: &str,
        visited: &mut Vec<String>,
        recursion_stack: &mut Vec<String>,
    ) -> bool {
        visited.push(node.to_string());
        recursion_stack.push(node.to_string());
        
        if let Some(node_data) = self.nodes.get(node) {
            for dep in &node_data.dependencies {
                if !visited.contains(dep) {
                    if self.detect_cycle_recursive(dep, visited, recursion_stack) {
                        return true;
                    }
                } else if recursion_stack.contains(dep) {
                    return true;
                }
            }
        }
        
        recursion_stack.retain(|n| n != node);
        false
    }

    /// Perform topological sort using Kahn's algorithm
    pub fn topological_sort(&mut self) -> Vec<String> {
        // Check cache
        if let Some(cached) = &self.sorted_cache {
            return cached.clone();
        }
        
        let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
        let mut queue: Vec<String> = Vec::new();
        let mut result: Vec<String> = Vec::new();
        
        // Calculate in-degrees
        for (name, node) in &self.nodes {
            let mut count = 0;
            for dep in &node.dependencies {
                if self.nodes.contains_key(dep) {
                    count += 1;
                }
            }
            in_degree.insert(name.clone(), count);
        }
        
        // Add all nodes with in-degree 0 to queue
        for (name, degree) in &in_degree {
            if *degree == 0 {
                queue.push(name.clone());
            }
        }
        
        // Process queue
        while let Some(node_name) = queue.pop() {
            result.push(node_name.clone());
            
            // Find all dependents and reduce their in-degree
            if let Some(node) = self.nodes.get(&node_name) {
                for dependent in &node.dependents {
                    if let Some(degree) = in_degree.get_mut(dependent) {
                        if *degree > 0 {
                            *degree -= 1;
                            if *degree == 0 {
                                queue.push(dependent.clone());
                            }
                        }
                    }
                }
            }
        }
        
        // Check if all nodes were processed
        if result.len() != self.nodes.len() {
            // There's a cycle
            result.clear();
        }
        
        // Cache result
        self.sorted_cache = Some(result.clone());
        
        result
    }

    /// Get nodes that can be started (all dependencies satisfied)
    pub fn get_startable_nodes(&self) -> Vec<String> {
        let mut startable = Vec::new();
        
        for (name, node) in &self.nodes {
            let mut all_deps_satisfied = true;
            for dep in &node.dependencies {
                if !self.nodes.contains_key(dep) {
                    // Missing dependency - can't start
                    all_deps_satisfied = false;
                    break;
                }
            }
            
            if all_deps_satisfied {
                startable.push(name.clone());
            }
        }
        
        startable
    }

    /// Get the number of nodes
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if graph is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Node in the dependency graph
struct Node {
    name: String,
    dependencies: Vec<String>, // Nodes this node depends on
    dependents: Vec<String>,   // Nodes that depend on this one
}

impl Node {
    fn new(name: String) -> Self {
        Node {
            name,
            dependencies: Vec::new(),
            dependents: Vec::new(),
        }
    }
}