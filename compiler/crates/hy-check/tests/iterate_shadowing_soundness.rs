#[test]
fn iterate_index_shadowing_collection_preserves_post_loop_share_edge() {
    let src = r#"
import "fmt"

fn worker(a [][]int) {
    a[0][0] = 99
}

fn main() {
    a := [][]int{[]int{1, 2}}
    saved := []int{0}
    for a, b in a {
        _ = a
        saved = b
    }
    task worker(move a)
    saved[0] = 42
    fmt.println(saved[0])
}
"#;
    let file = hy_syntax::parse_file(src, "test.hy").expect("parse");
    let res = hy_check::check_file(&file);
    assert_eq!(res.verdict, hy_check::Verdict::Reject);
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3003"));
}

#[test]
fn iterate_self_shadowing_preserves_outer_alias_freeze() {
    let src = r#"
import "fmt"

fn worker(xs [][]int) {
    xs[0][0] = 99
}

fn main() {
    xs := [][]int{[]int{1, 2}}
    alias := xs
    for xs in xs {
        task worker(move alias)
        xs[0] = 42
        fmt.println(xs[0])
        break
    }
}
"#;
    let file = hy_syntax::parse_file(src, "test.hy").expect("parse");
    let res = hy_check::check_file(&file);
    assert_eq!(res.verdict, hy_check::Verdict::Reject);
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3003"));
}

#[test]
fn iterate_self_shadowing_const_rejects_mutation() {
    for src in [
        r#"
import "fmt"
const table = [][]int{[]int{1, 2}}
fn main() {
    for table in table {
        table[0] = 99
        fmt.println(table[0])
    }
}
"#,
        r#"
import "fmt"
const table = [][]int{[]int{1, 2}}
fn main() {
    for table, row in table {
        _ = table
        row[0] = 99
        fmt.println(row[0])
    }
}
"#,
    ] {
        let file = hy_syntax::parse_file(src, "test.hy").expect("parse");
        let res = hy_check::check_file(&file);
        assert_eq!(res.verdict, hy_check::Verdict::Reject);
        assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3006"));
    }
}
