#[derive(Debug)]
pub enum GeometryType {
    Geometry {
        name: String,
        children: GeometryVector,
    },
    Reference {
        name: String,
        reference: String,
        children: GeometryVector,
    }
}

pub type GeometryVector = Vec<GeometryType>;