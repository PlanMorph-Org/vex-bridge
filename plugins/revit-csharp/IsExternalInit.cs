// Polyfill for C# 9+ `init` accessors and `record` types when targeting
// .NET Framework 4.8. The compiler looks for this exact type name; if
// it's absent in the target framework, we just need to declare it so
// the compiler-generated code resolves.
//
// See: https://learn.microsoft.com/dotnet/csharp/language-reference/proposals/csharp-9.0/init
namespace System.Runtime.CompilerServices
{
    internal static class IsExternalInit { }
}
