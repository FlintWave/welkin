use nalgebra::{Matrix4, Vector3};

#[derive(Debug, Clone, Default)]
pub struct SkeletonJoint {
    pub name: String,
    pub parent: i32,
    pub position: Vector3<f32>,
    pub bind_matrix: Matrix4<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct MeshSkeleton {
    pub joints: Vec<SkeletonJoint>,
}

impl MeshSkeleton {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        // Minimal placeholder parser: expects little-endian count followed by
        // (name length + name + parent i32 + xyz f32 + 16 f32 bind matrix).
        use std::io::Cursor;
        use byteorder::{LittleEndian, ReadBytesExt};

        let mut c = Cursor::new(bytes);
        let count = c.read_u32::<LittleEndian>().ok()? as usize;
        let mut joints = Vec::with_capacity(count);
        for _ in 0..count {
            let nlen = c.read_u32::<LittleEndian>().ok()? as usize;
            let mut namebuf = vec![0u8; nlen];
            std::io::Read::read_exact(&mut c, &mut namebuf).ok()?;
            let name = String::from_utf8(namebuf).ok()?;
            let parent = c.read_i32::<LittleEndian>().ok()?;
            let x = c.read_f32::<LittleEndian>().ok()?;
            let y = c.read_f32::<LittleEndian>().ok()?;
            let z = c.read_f32::<LittleEndian>().ok()?;
            let mut bind = Matrix4::identity();
            for r in 0..4 {
                for col in 0..4 {
                    bind[(r, col)] = c.read_f32::<LittleEndian>().ok()?;
                }
            }
            joints.push(SkeletonJoint {
                name,
                parent,
                position: Vector3::new(x, y, z),
                bind_matrix: bind,
            });
        }
        Some(MeshSkeleton { joints })
    }
}