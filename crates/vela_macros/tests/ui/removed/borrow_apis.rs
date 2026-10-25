use vela_engine::runtime::CallArgs;

fn main() {
    let mut values = vec![1_i64];
    let _ = CallArgs::new().with_host_ref("values", &values);
    let _ = CallArgs::new().with_host_mut("values", &mut values);
    let mut args = CallArgs::new();
    args.push_host_ref("values", &values);
    args.push_host_mut("values", &mut values);
}
