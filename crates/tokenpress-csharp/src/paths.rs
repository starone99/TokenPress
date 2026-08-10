//! Which paths the C# backend claims.
//!
//! A plain extension check, exactly Go's and Java's shape and unlike Ruby's
//! basename matching. C# has no file whose *name* makes it special: an
//! `AssemblyInfo.cs`, a generated `*.Designer.cs` and a `*.g.cs` are all
//! ordinary `.cs` sources, `.csproj` and `.sln` are project metadata rather
//! than source, `.csx` scripting files are a different dialect this backend
//! does not claim, and `.vb` / `.fs` are other languages on the same runtime.
//! There is therefore no basename matching here, and nothing this module has
//! to know about a project layout.
use std::path::Path;
/// True when `path` names a C# source file.
///
/// Accepted by extension, and by extension only: `cs`. The match is
/// **case-sensitive**, the same convention the Go, Ruby and Java backends
/// use: the C# toolchain looks for `.cs`, so `A.CS` is not a C# file to it
/// and is not one here either.
pub fn supports_path(path:&Path)->bool{path.extension().is_some_and(|e|e.to_str()==Some("cs"))}#[cfg(test)]mod tests{use super::*;#[test]fn accepts_the_csharp_extension(){for name in["A.cs","Program.cs","src/Newtonsoft.Json/Linq/JToken.cs","/abs/path/A.cs","AssemblyInfo.cs","Thing.Designer.cs","Thing.g.cs","A.b.cs",]{assert!(supports_path(Path::new(name)),"{name} should be supported");}}#[test]fn rejects_other_languages_and_near_misses(){let other_languages=["a.go","a.java","a.py","a.rs","a.rb","a.txt"];let case_variants=["A.CS","A.Cs","A.cS"];let near_misses=["A.csx","A.c","A.cshtml","A.cs.txt","A."];let not_source=["A.csproj","A.sln","A.dll","A.vb","A.fs","cs","Makefile","foo",];for name in other_languages.iter().chain(&case_variants).chain(&near_misses).chain(&not_source){assert!(!supports_path(Path::new(name)),"{name} should be rejected");}}#[test]fn rejects_a_path_with_no_file_name(){assert!(!supports_path(Path::new("..")));}}