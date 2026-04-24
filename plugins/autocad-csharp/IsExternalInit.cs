// IsExternalInit polyfill — required for `record struct` and `init` accessors
// on .NET Framework 4.8 (which is the AutoCAD 2024-and-earlier TFM). The
// modern TFMs (net8.0-windows, net10.0-windows) include this type already
// and ignore the duplicate via internal visibility.
namespace System.Runtime.CompilerServices
{
    using System.ComponentModel;

    [EditorBrowsable(EditorBrowsableState.Never)]
    internal static class IsExternalInit { }
}
