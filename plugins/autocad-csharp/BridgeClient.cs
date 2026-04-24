using System;
using System.IO;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace Architur.VexBridgeAutoCAD;

/// <summary>
/// Tiny HTTP client over the local vex-bridge daemon (loopback only).
/// Identical to the Revit plugin's BridgeClient — talks to the same
/// daemon. See plugins/revit-csharp/BridgeClient.cs for design notes.
/// </summary>
internal sealed class BridgeClient : IDisposable
{
    private const string BaseUrl = "http://127.0.0.1:7878";
    private static readonly TimeSpan Timeout = TimeSpan.FromMinutes(5);

    private readonly HttpClient _http;

    private BridgeClient(string token)
    {
        _http = new HttpClient { Timeout = Timeout };
        _http.DefaultRequestHeaders.Add("X-Vex-Bridge-Token", token);
    }

    public static BridgeClient? TryCreate()
    {
        var token = ReadToken();
        return string.IsNullOrEmpty(token) ? null : new BridgeClient(token!);
    }

    public bool IsPaired()
    {
        try
        {
            var r = _http.GetAsync($"{BaseUrl}/v1/pair/status").GetAwaiter().GetResult();
            if (!r.IsSuccessStatusCode) return false;
            var body = r.Content.ReadAsStringAsync().GetAwaiter().GetResult();
            return body.Contains("\"status\":\"paired\"", StringComparison.Ordinal);
        }
        catch { return false; }
    }

    public PairStart StartPair(string deviceLabel)
    {
        var body = JsonSerializer.Serialize(new { device_label = deviceLabel });
        using var content = new StringContent(body, Encoding.UTF8, "application/json");
        var r = _http.PostAsync($"{BaseUrl}/v1/pair/start", content).GetAwaiter().GetResult();
        var text = r.Content.ReadAsStringAsync().GetAwaiter().GetResult();
        if (!r.IsSuccessStatusCode) throw new BridgeException((int)r.StatusCode, text);
        using var doc = JsonDocument.Parse(text);
        var el = doc.RootElement;
        return new PairStart(
            el.GetProperty("code").GetString() ?? "",
            el.GetProperty("pair_url").GetString() ?? "",
            el.TryGetProperty("expires_at", out var ea) ? ea.GetString() ?? "" : "");
    }

    public bool WaitForPaired(CancellationToken ct, TimeSpan timeout)
    {
        var deadline = DateTime.UtcNow + timeout;
        while (!ct.IsCancellationRequested && DateTime.UtcNow < deadline)
        {
            if (IsPaired()) return true;
            try { Task.Delay(TimeSpan.FromSeconds(2), ct).Wait(ct); }
            catch (OperationCanceledException) { return false; }
        }
        return false;
    }

    public string Register(string projectId, string? localPath = null)
    {
        var body = JsonSerializer.Serialize(new { project_id = projectId, local_path = localPath });
        using var content = new StringContent(body, Encoding.UTF8, "application/json");
        var r = _http.PostAsync($"{BaseUrl}/v1/repo/register", content).GetAwaiter().GetResult();
        var text = r.Content.ReadAsStringAsync().GetAwaiter().GetResult();
        if (!r.IsSuccessStatusCode) throw new BridgeException((int)r.StatusCode, text);
        return text;
    }

    public string Push(string projectId, string? branch)
    {
        var body = JsonSerializer.Serialize(new { project_id = projectId, branch });
        using var content = new StringContent(body, Encoding.UTF8, "application/json");
        var r = _http.PostAsync($"{BaseUrl}/v1/repo/push", content).GetAwaiter().GetResult();
        var text = r.Content.ReadAsStringAsync().GetAwaiter().GetResult();
        if (!r.IsSuccessStatusCode) throw new BridgeException((int)r.StatusCode, text);
        return text;
    }

    public void Dispose() => _http.Dispose();

    private static string? ReadToken()
    {
        var path = Environment.OSVersion.Platform == PlatformID.Win32NT
            ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                           "vex-bridge", "access-token")
            : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                           ".config", "vex-bridge", "access-token");
        return File.Exists(path) ? File.ReadAllText(path).Trim() : null;
    }
}

internal readonly record struct PairStart(string Code, string PairUrl, string ExpiresAt);

internal sealed class BridgeException : Exception
{
    public int StatusCode { get; }
    public BridgeException(int statusCode, string body)
        : base($"vex-bridge returned HTTP {statusCode}: {body}")
    {
        StatusCode = statusCode;
    }
}
