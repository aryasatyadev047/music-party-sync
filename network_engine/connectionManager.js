class ConnectionManager {
    constructor() {
        this.devices = new Map();
    }

    addDevice(deviceId, rinfo) {
        if (!this.devices.has(deviceId)) {
            this.devices.set(deviceId, {
                status: 'Connected',
                address: rinfo.address,
                port: rinfo.port,
                latency: 0,
                lastSeen: Date.now()
            });
            console.log(`🔗 [Connection Manager] Device added: ${deviceId}`);
        }
    }

    removeDevice(deviceId) {
        if (this.devices.has(deviceId)) {
            this.devices.delete(deviceId);
            console.log(`❌ [Connection Manager] Device removed: ${deviceId}`);
        }
    }

    updateLatency(deviceId, latencyMs) {
        let device = this.devices.get(deviceId);
        if (device) {
            device.latency = latencyMs;
            device.lastSeen = Date.now();
        }
    }

    getConnectedDevices() {
        return Array.from(this.devices.entries());
    }
}

module.exports = ConnectionManager;
