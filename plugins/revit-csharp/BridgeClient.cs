using System;
using System.IO;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace Architur.VexBridgeRevit;

/// <summary>
/// Tiny HTTP client over the local vex-bridge daemon (loopback only).
/// Reads the access token from the per-user config dir, talks to
/// http://127.0.0.1:7878/v1/*, and surfaces ApiError JSON envelopes as
/// thrown <see cref="BridgeException"/>s.
///
/// Endpoints (see crates/vex-bridge/src/server.rs):
///   GET  /v1/health        -- no auth, used to detect the daemon
///   GET  /v1/pair/status   -- token, returns Unpaired|Pending|Paired
///   POST /v1/pair/start    -- token, kicks off pairing, returns code+url
///   POST /v1/repo/push     -- token, manual push (added to daemon by us)
///
/// Kept synchronous-friendly because Revit external commands run on the
/// UI thread.
/// </summary>
internal sealed class BridgeClient : IDisposable
{
    private const string BaseUrl = "http://127.0.0.1:7878";
    private static readonly TimeSpan Timeout = TimeSpan.FromMinutes(5);

    private readonly HttpClient _http;
    private readonly string _token;

    private BridgeClient(string token)
    {
        _http = new HttpClient { Timeout = Timeout };
        _http.DefaultRequestHeaders.Add("X-Vex-Bridge-Token", token);
        _token = token;
    }

    /// <summary>
    /// Returns null if the access token file isn't present yet (which is
    /// normal before the daemon has been started for the first time —
    /// the daemon writes it on first launch). Callers should call
    /// <see cref="BundledBin.EnsureDaemonRunning"/> first.
    /// </summary>
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

    /// <summary>
    /// Starts pairing (POST /v1/pair/start). Returns the human-readable
    /// code, the verification URL the user should open, and the absolute
    /// expiry time.
    /// </summary>
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

    /// <summary>
    /// Polls /v1/pair/status until paired, expired, or the caller cancels.
    /// Designed to be called from a background thread; returns true on
    /// successful pairing, false on expiry / cancellation.
    /// </summary>
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

    /// <summary>
    /// Idempotently register a project with the daemon. Safe to call
    /// before every Push — if the project is already registered the
    /// daemon just confirms with `replaced: true`. Returns the
    /// daemon-resolved local path so callers can show it to the user.
    /// </summary>
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
        if (!r.IsSuccessStatusCode)
            throw new BridgeException((int)r.StatusCode, text);
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
