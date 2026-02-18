// Nanite Streaming Feedback
//
// Collects peak usage data from cull counters for streaming decisions.
// Most streaming logic is CPU-side; this shader handles GPU-side aggregation.
//
// Entry points:
//   reset_feedback  — Zero out feedback counters before a new frame.
//   update_peak     — Compare current usage against capacity thresholds.

struct FeedbackParams {
    max_visible_clusters: u32,
    max_nodes: u32,
    frame_index: u32,
    _pad: u32,
}

struct StreamingFeedback {
    visible_cluster_peak: atomic<u32>,
    node_peak: atomic<u32>,
    requested_pages: atomic<u32>,
    total_resident_pages: u32,
}

@group(0) @binding(0) var<uniform> params: FeedbackParams;
@group(0) @binding(1) var<storage, read_write> feedback: StreamingFeedback;

@compute @workgroup_size(1)
fn reset_feedback(@builtin(global_invocation_id) gid: vec3<u32>) {
    atomicStore(&feedback.visible_cluster_peak, 0u);
    atomicStore(&feedback.node_peak, 0u);
    atomicStore(&feedback.requested_pages, 0u);
}

@compute @workgroup_size(1)
fn update_peak(
    @builtin(global_invocation_id) gid: vec3<u32>,
) {
    // Compare current usage against thresholds and signal overflow
    let cluster_peak = atomicLoad(&feedback.visible_cluster_peak);
    let node_peak = atomicLoad(&feedback.node_peak);

    // Signal overflow by incrementing requested_pages — the CPU reads
    // this counter to detect that capacity was exceeded this frame.
    if cluster_peak > params.max_visible_clusters {
        // How many clusters over budget: gives CPU a magnitude signal
        let overshoot = cluster_peak - params.max_visible_clusters;
        atomicAdd(&feedback.requested_pages, overshoot);
    }
    if node_peak > params.max_nodes {
        let overshoot = node_peak - params.max_nodes;
        atomicAdd(&feedback.requested_pages, overshoot);
    }
}
