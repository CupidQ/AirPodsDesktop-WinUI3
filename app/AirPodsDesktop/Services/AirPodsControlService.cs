namespace AirPodsDesktop.Services;

/// <summary>Serializes device operations off the WinUI thread, across page lifetimes.</summary>
public sealed class AirPodsControlService
{
    private readonly SemaphoreSlim _gate = new(1, 1);
    private bool _closed;

    private async Task<T> RunAsync<T>(Func<T> operation)
    {
        await _gate.WaitAsync().ConfigureAwait(false);
        try
        {
            if (_closed) throw new ObjectDisposedException(nameof(AirPodsControlService));
            return await Task.Run(operation).ConfigureAwait(false);
        }
        finally { _gate.Release(); }
    }

    public Task<ControlDevices> ListDevicesAsync() => RunAsync(Native.ControlListDevices);
    public Task<ControlSnapshot> StateAsync() => RunAsync(Native.ControlState);
    public Task<ControlResult> ConnectAsync(string path) => RunAsync(() => Native.ControlConnect(path));
    public Task<ControlResult> DisconnectAsync() => RunAsync(Native.ControlDisconnect);
    public Task<ControlResult> SetAsync(uint setting, uint value) => RunAsync(() => Native.ControlSet(setting, value));
    public Task<ControlResult> SetDuckAsync(bool enabled) => RunAsync(() => Native.ControlSetDuck(enabled));

    /// <summary>Finish native teardown before process exit so ducked volume is restored.</summary>
    public void Shutdown()
    {
        _gate.Wait();
        try
        {
            if (_closed) return;
            _closed = true;
            Task.Run(Native.ControlDisconnect).GetAwaiter().GetResult();
        }
        finally { _gate.Release(); }
    }
}
