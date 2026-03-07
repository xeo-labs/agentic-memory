use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn mcp_json_parse(c: &mut Criterion) {
    c.bench_function("mcp_json_parse", |b| {
        b.iter(|| {
            let json = r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"test"},"id":1}"#;
            black_box(serde_json::from_str::<serde_json::Value>(json).unwrap());
        })
    });
}

fn mcp_tool_dispatch(c: &mut Criterion) {
    c.bench_function("mcp_tool_dispatch", |b| {
        b.iter(|| {
            let tools = ["tool1", "tool2", "tool3", "tool4", "tool5"];
            let target = "tool3";
            black_box(tools.iter().find(|&&t| t == target));
        })
    });
}

criterion_group!(benches, mcp_json_parse, mcp_tool_dispatch);
criterion_main!(benches);
