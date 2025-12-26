// Simple test program to verify .skope file loading

mod skope_data;

fn main() {
    println!("=== Testing .skope File Loader ===\n");

    // Test 1: Load player.skope
    println!("Test 1: Loading player.skope...");
    match skope_data::EntityPrefab::from_file("test_entities/player.skope") {
        Ok(prefab) => {
            println!("✓ SUCCESS: Loaded '{}'", prefab.name);
            println!("  Components: {}", prefab.components.len());

            // Print components
            for (i, component) in prefab.components.iter().enumerate() {
                match component {
                    skope_data::Component::Transform(t) => {
                        println!("    [{}] Transform: pos({}, {}, {})",
                            i, t.position.x, t.position.y, t.position.z);
                    }
                    skope_data::Component::Mesh(m) => {
                        println!("    [{}] Mesh: {}", i, m.asset);
                    }
                    skope_data::Component::Health(h) => {
                        println!("    [{}] Health: {}/{}", i, h.current, h.max);
                    }
                    skope_data::Component::Chroma(c) => {
                        println!("    [{}] Chroma: {}/{}", i, c.current, c.max);
                    }
                }
            }
        }
        Err(e) => {
            println!("✗ FAILED: {}", e);
        }
    }

    println!();

    // Test 2: Load test_cube.skope
    println!("Test 2: Loading test_cube.skope...");
    match skope_data::EntityPrefab::from_file("test_entities/test_cube.skope") {
        Ok(prefab) => {
            println!("✓ SUCCESS: Loaded '{}'", prefab.name);
            println!("  Components: {}", prefab.components.len());
        }
        Err(e) => {
            println!("✗ FAILED: {}", e);
        }
    }

    println!("\n=== Test Complete ===");
}
