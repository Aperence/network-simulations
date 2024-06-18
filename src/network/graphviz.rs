use std::{collections::HashMap, fmt::Display};

pub enum EdgeOption{
    Color(String),
    FontColor(String),
    Label(String),
    Arrowhead(String),
    Headlabel(String),
    Taillabel(String)
}

impl Display for EdgeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeOption::Color(c) => write!(f, "color={}", c),
            EdgeOption::FontColor(c) => write!(f, "fontcolor={}", c),
            EdgeOption::Label(l) => write!(f, "label=\"{}\"", l),
            EdgeOption::Arrowhead(t) => write!(f, "arrowhead={}", t),
            EdgeOption::Headlabel(l) => write!(f, "headlabel=\"{}\"", l),
            EdgeOption::Taillabel(l) => write!(f, "taillabel=\"{}\"", l),
        }
    }
}

pub enum NodeOption{
    Shape(String),
}

impl Display for NodeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeOption::Shape(shape) => write!(f, "shape={}", shape),
        }
    }
}

pub enum GraphOption{
    NodeSep(String),
    RankSep(String),
}

impl Display for GraphOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphOption::NodeSep(size) => write!(f, "nodesep=\"{}\"", size),
            GraphOption::RankSep(size) => write!(f, "ranksep=\"{}\"", size), 
        }
    }
}

type GroupNodes = Vec<(String, Vec<NodeOption>)>; // name, nodeOptions

pub struct Graph{
    nodes: Vec<(String, Vec<NodeOption>)>,
    groups: HashMap<String, (String, GroupNodes)>,
    edges: Vec<(String, String, Vec<EdgeOption>)>,
    graph_options: Vec<GraphOption>
}

impl Graph{
    pub fn new(options: Vec<GraphOption>) -> Graph{
        Graph{nodes: vec![], groups: HashMap::new(), edges: vec![], graph_options: options}
    }

    pub fn add_node(&mut self, name: &str, options: Vec<NodeOption>){
        self.nodes.push((name.to_string(), options));
    }

    pub fn add_group(&mut self, name: &str, label: &str){
        self.groups.insert(name.to_string(), (label.to_string(), vec![]));
    }

    pub fn add_node_group(&mut self, name: &str, group: &str, options: Vec<NodeOption>){
        self.groups.get_mut(group).unwrap().1.push((name.to_string(), options));
    }

    pub fn add_edge(&mut self, node1: &str, node2: &str, options: Vec<EdgeOption>){
        self.edges.push((node1.to_string(), node2.to_string(), options));
    }
}

impl Display for Graph{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = String::new();
        string.push_str("digraph{\n");
        
        string.push_str(&format!(" graph[{}];\n", self.graph_options.iter().map(|o| format!("{}", o)).collect::<Vec<String>>().join(",")));

        for (node, node_options) in self.nodes.iter(){
            string.push_str(
                &format!("  {}[{}];\n", node, node_options.iter().map(|o| format!("{}", o)).collect::<Vec<String>>().join(",")));
        }

        for (group, (group_label, nodes)) in self.groups.iter(){
            string.push_str(
                &format!("  subgraph cluster_{} {{\n", group));
            string.push_str(
                &format!("    label=\"{}\";\n", group_label));
            for (node, node_options) in nodes{
                string.push_str(
                    &format!("    {}[{}];\n", node, node_options.iter().map(|o| format!("{}", o)).collect::<Vec<String>>().join(",")));
            }
            string.push_str(
                &format!("  }}\n"));
        }

        for (node1, node2, options) in self.edges.iter(){
            string.push_str(
                &format!("  {} -> {}[{}];\n", node1, node2, options.iter().map(|o| format!("{}", o)).collect::<Vec<String>>().join(",")));
        }

        string.push_str("}");
        write!(f, "{}", string)
    }
}