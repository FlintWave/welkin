#[cfg(test)]
mod tests {
    use welkin::simulator::Simulator;
    use welkin::object::Prim;
    use welkin::script::Script;

    #[test]
    fn rez_animesh_and_set_animation() {
        // Initialize a simple simulator instance
        let mut sim = Simulator::new();

        // Rez an animesh primitive at the origin
        let prim = Prim::new_animesh();
        let prim_id = sim.rez_prim(prim, (0.0, 0.0, 0.0));

        // Attach a simple script that sets an animation
        let script = Script::new("
            default
            {
                state_entry()
                {
                    llSetAnimation(\"walk\");
                }
            }
        ");
        sim.attach_script(prim_id, script);

        // Run a few simulation ticks to allow the script to execute
        for _ in 0..5 {
            sim.tick();
        }

        // Verify that the prim now has an animation assigned
        let anim_state = sim.get_animation_state(prim_id);
        assert!(anim_state.is_some(), "Animation state should be set");
        assert_eq!(anim_state.unwrap(), "walk");
    }
}