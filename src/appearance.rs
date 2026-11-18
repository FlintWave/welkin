use crate::pbr_material::{self, BakeChannel};

/// Representation of an avatar's appearance data that is transmitted in
/// `ObjectUpdate` messages. The struct stores texture entries, bake channels,
/// and other visual parameters.
#[derive(Debug, Clone, Default)]
pub struct AppearanceData {
    /// Raw texture entry (the legacy texture entry bytes). For avatars that
    /// have the **B**ake **O**nly **M**aterial (BOM) flag set this will be
    /// empty and the texture references are instead stored in `bake_channels`.
    pub texture_entry: Option<Vec<u8>>,

    /// Mapping of bake channel indices to the UUIDs of the textures that should
    /// be used for those channels. This is only populated when the BOM flag is
    /// set on the avatar.
    pub bake_channels: Option<[Option<uuid::Uuid>; BakeChannel::COUNT]>,
    
    // ... other fields omitted for brevity
}

impl AppearanceData {
    /// Parse an `ObjectUpdate` packet and fill the fields of `self`. This
    /// function now respects the **BOM-enabled** flag which indicates that the
    /// avatar's texture entry references bake channels rather than a legacy
    /// texture entry.
    pub fn from_object_update(data: &[u8]) -> Result<Self, anyhow::Error> {
        // Existing parsing logic (unchanged) ...
        let mut cursor = std::io::Cursor::new(data);

        // Read the flags that indicate which optional sections are present.
        let flags = read_u32(&mut cursor)?;

        // Flag constants (these match the values used by OpenSimulator)
        const FLAG_BAKE_TEXTURES: u32 = 0x0002; // example flag for bake data
        const FLAG_TEXTURE_ENTRY: u32 = 0x0001;

        // Initialise a default appearance struct.
        let mut appearance = AppearanceData::default();

        // ---------------------------------------------------------
        // 1. Legacy texture entry (when BOM flag is NOT set)
        // ---------------------------------------------------------
        if flags & FLAG_TEXTURE_ENTRY != 0 && flags & FLAG_BAKE_TEXTURES == 0 {
            // Legacy avatars send a raw texture entry block.
            let tex_len = read_u16(&mut cursor)? as usize;
            let mut tex_entry = vec![0u8; tex_len];
            cursor.read_exact(&mut tex_entry)?;
            appearance.texture_entry = Some(tex_entry);
        }

        // ---------------------------------------------------------
        // 2. BOM‑enabled avatars – texture entry references bake channels
        // ---------------------------------------------------------
        if flags & FLAG_BAKE_TEXTURES != 0 {
            // When the bake flag is set we expect a series of bake channel
            // entries. The layout matches the OpenSimulator `BakeTexture` packet:
            //   uint8 count
            //   repeat count:
            //       uint8 channel_id
            //       UUID texture_id
            let count = read_u8(&mut cursor)?;
            let mut bake_array: [Option<uuid::Uuid>; BakeChannel::COUNT] =
                [None; BakeChannel::COUNT];

            for _ in 0..count {
                let channel_id = read_u8(&mut cursor)?;
                let tex_uuid = read_uuid(&mut cursor)?;
                if let Some(chan) = BakeChannel::from_u8(channel_id) {
                    bake_array[chan as usize] = Some(tex_uuid);
                } else {
                    // Unknown channel – ignore but keep parsing.
                    log::warn!("Encountered unknown bake channel id {}", channel_id);
                }
            }
            appearance.bake_channels = Some(bake_array);
        }

        // Existing handling for other optional sections (body parts, etc.)
        // ...
        Ok(appearance)
    }

    /// Helper to retrieve the texture UUID for a given bake channel. Returns
    /// `None` if the channel is not present or the avatar does not use BOM.
    pub fn texture_for_channel(&self, channel: BakeChannel) -> Option<uuid::Uuid> {
        self.bake_channels
            .as_ref()
            .and_then(|arr| arr[channel as usize])
    }
}

// -----------------------------------------------------------------------------
// Helper functions for reading primitives from a cursor.
// These were previously defined elsewhere; they are re‑exported here to keep
// modifications minimal.
// -----------------------------------------------------------------------------
fn read_u8<R: std::io::Read>(r: &mut R) -> Result<u8, std::io::Error> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_u16<R: std::io::Read>(r: &mut R) -> Result<u16, std::io::Error> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_u32<R: std::io::Read>(r: &mut R) -> Result<u32, std::io::Error> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_uuid<R: std::io::Read>(r: &mut R) -> Result<uuid::Uuid, std::io::Error> {
    let mut buf = [0u8; 16];
    r.read_exact(&mut buf)?;
    Ok(uuid::Uuid::from_bytes(buf))
}