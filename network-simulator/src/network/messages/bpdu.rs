#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct BPDU{
    pub root: u32,
    pub distance: u32,
    pub switch: u32,
    pub port: u32
}

impl ToString for BPDU{
    fn to_string(&self) -> String{
        format!("<{},{},{},{}>", self.root, self.distance, self.switch, self.port)
    }
}