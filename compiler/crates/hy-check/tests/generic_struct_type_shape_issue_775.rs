//! Regression tests for issue #775:
//! Generic struct instantiations (`box[T]`, `pair[K, V]`) preserve their full
//! instantiated type arguments in `TypeShape::Struct(Some("box[T]"))` and
//! substitute type parameters when resolving field types across `shape_may_share`,
//! `field_shape`, `shape_holds_closure`, `shape_holds_unmovable_leaf`,
//! `shape_is_const_sendable`, `field_borrow_kind`, `ty_of_expr`, and
//! `place_target_ty`.

use hy_check::{check_file, Verdict};
use hy_syntax::parse_file;

fn check(src: &str) -> hy_check::CheckResult {
    let file = parse_file("test.hy", src).expect("parse should succeed");
    check_file(&file)
}

#[test]
fn generic_struct_shallow_copy_shares_slice_and_map_interior_issue_775() {
    let res_slice = check(
        r#"
type box[T any] struct {
  v T
}

fn worker_slice(b box[[]int]) {
  b.v[0] = 99
}

fn main() {
  b1 := box[[]int]{v: []int{1, 2, 3}}
  b2 := b1
  task worker_slice(move b1)
  b2.v[0] = 42
}
"#,
    );
    assert_eq!(
        res_slice.verdict,
        Verdict::Reject,
        "expected Reject, got: {:?}",
        res_slice.diagnostics
    );
    assert!(
        res_slice.diagnostics.iter().any(|d| d.code == "HYB-E3003"),
        "expected HYB-E3003 on post-move write through shallow copy `b2`, got: {:?}",
        res_slice.diagnostics
    );

    let res_map = check(
        r#"
type box[T any] struct {
  v T
}

fn worker_map(b box[map[string]int]) {
  b.v["k"] = 99
}

fn main() {
  b1 := box[map[string]int]{v: map[string]int{"k": 1}}
  b2 := b1
  task worker_map(move b1)
  b2.v["k"] = 42
}
"#,
    );
    assert_eq!(
        res_map.verdict,
        Verdict::Reject,
        "expected Reject, got: {:?}",
        res_map.diagnostics
    );
    assert!(
        res_map.diagnostics.iter().any(|d| d.code == "HYB-E3003"),
        "expected HYB-E3003 on post-move map write through `b2`, got: {:?}",
        res_map.diagnostics
    );
}

#[test]
fn generic_struct_field_projection_propagates_alias_and_non_owned_origin_issue_775() {
    let res_alias = check(
        r#"
type box[T any] struct {
  v T
}

fn worker(b box[[]int]) {
  b.v[0] = 1
}

fn main() {
  b := box[[]int]{v: []int{1, 2}}
  s := b.v
  task worker(move b)
  s[0] = 2
}
"#,
    );
    assert_eq!(
        res_alias.verdict,
        Verdict::Reject,
        "expected Reject, got: {:?}",
        res_alias.diagnostics
    );
    assert!(
        res_alias.diagnostics.iter().any(|d| d.code == "HYB-E3003"),
        "expected HYB-E3003 on `s[0] = 2` after moving `b`, got: {:?}",
        res_alias.diagnostics
    );

    let res_param = check(
        r#"
type box[T any] struct {
  v T
}

fn worker(s []int) {
  s[0] = 1
}

fn leak(b box[[]int]) {
  s := b.v
  task worker(move s)
}
"#,
    );
    assert_eq!(
        res_param.verdict,
        Verdict::Reject,
        "expected Reject, got: {:?}",
        res_param.diagnostics
    );
    assert!(
        res_param.diagnostics.iter().any(|d| d.code == "HYB-E3004"),
        "expected HYB-E3004 when moving slice projected from non-owned generic struct param, got: {:?}",
        res_param.diagnostics
    );
}

#[test]
fn generic_struct_const_binding_rejects_interior_write_and_task_send_issue_775() {
    let res_const_write = check(
        r#"
type box[T any] struct {
  v T
}

fn mutate_const() {
  const b = box[[]int]{v: []int{1, 2}}
  s := b.v
  s[0] = 99
}
"#,
    );
    assert_eq!(
        res_const_write.verdict,
        Verdict::Reject,
        "expected Reject, got: {:?}",
        res_const_write.diagnostics
    );
    assert!(
        res_const_write
            .diagnostics
            .iter()
            .any(|d| d.code == "HYB-E3006"),
        "expected HYB-E3006 on interior write to `const b` via `s := b.v`, got: {:?}",
        res_const_write.diagnostics
    );

    let res_const_send = check(
        r#"
type box[T any] struct {
  v T
}

fn worker(b box[[]int]) {
  b.v[0] = 99
}

fn main() {
  const b = box[[]int]{v: []int{1, 2}}
  task worker(b)
}
"#,
    );
    assert_eq!(
        res_const_send.verdict,
        Verdict::Reject,
        "expected Reject, got: {:?}",
        res_const_send.diagnostics
    );
    assert!(
        res_const_send
            .diagnostics
            .iter()
            .any(|d| d.code == "HYB-E3001"),
        "expected HYB-E3001 when sending `const box[[]int]` without `move`, got: {:?}",
        res_const_send.diagnostics
    );
}

#[test]
fn generic_struct_holding_closure_or_opaque_rejected_at_move_boundary_issue_775() {
    let res_closure = check(
        r#"
type box[T any] struct {
  v T
}

fn worker(b box[fn()]) {
  b.v()
}

fn main() {
  x := 0
  b := box[fn()]{v: fn() { x = 1 }}
  task worker(move b)
}
"#,
    );
    assert_eq!(
        res_closure.verdict,
        Verdict::Reject,
        "expected Reject, got: {:?}",
        res_closure.diagnostics
    );
    assert!(
        res_closure
            .diagnostics
            .iter()
            .any(|d| d.code == "HYB-E3004"),
        "expected HYB-E3004 when moving `box[fn()]` across a task boundary, got: {:?}",
        res_closure.diagnostics
    );

    let res_opaque = check(
        r#"
import "sync"

type box[T any] struct {
  v T
}

fn worker(b box[sync.Mutex]) {}

fn main() {
  b := box[sync.Mutex]{}
  task worker(move b)
}
"#,
    );
    assert_eq!(
        res_opaque.verdict,
        Verdict::Reject,
        "expected Reject, got: {:?}",
        res_opaque.diagnostics
    );
    assert!(
        res_opaque.diagnostics.iter().any(|d| d.code == "HYB-E3004"),
        "expected HYB-E3004 when moving `box[sync.Mutex]` across a task boundary, got: {:?}",
        res_opaque.diagnostics
    );
}

#[test]
fn generic_struct_instantiated_with_pointer_or_borrow_handle_rejected_issue_775() {
    let res_ptr = check(
        r#"
type box[T any] struct {
  v T
}

fn bad() {
  x := 1
  b := box[*int]{v: &x}
  _ = b
}
"#,
    );
    assert_eq!(
        res_ptr.verdict,
        Verdict::Reject,
        "expected Reject, got: {:?}",
        res_ptr.diagnostics
    );
    assert!(
        res_ptr.diagnostics.iter().any(|d| d.code == "HYB-E3103"),
        "expected HYB-E3103 on `box[*int]`, got: {:?}",
        res_ptr.diagnostics
    );
}

#[test]
fn pure_value_generic_struct_copy_and_const_send_still_accepted_issue_775() {
    let res = check(
        r#"
type box[T any] struct {
  v T
}

fn worker(b box[int]) {}

fn main() {
  b1 := box[int]{v: 10}
  b2 := b1
  task worker(move b1)
  b2.v = 20

  const cb = box[int]{v: 42}
  task worker(cb)
}
"#,
    );
    assert_eq!(
        res.verdict,
        Verdict::Accept,
        "expected Accept for pure-value `box[int]`, got: {:?}",
        res.diagnostics
    );
}
