use vela_macros::export;

#[export(path = "removed::host")]
pub fn host(_value: Host<Record>) {}

#[export(path = "removed::view")]
pub fn view(_value: View<i64>) {}

#[export(path = "removed::mut_view")]
pub fn mut_view(_value: MutView<i64>) {}

#[export(path = "removed::lease")]
pub fn lease(_value: HostLeaseRef<Record>) {}

#[export(path = "removed::path")]
pub fn path(_value: Option<HostPath>) {}

#[export(path = "removed::nested")]
pub fn nested(_value: Vec<PathProxy>) {}

fn main() {}
