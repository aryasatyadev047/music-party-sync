const DiscoveryModule = require('./discovery');
const TransportModule = require('./transport');
const ConnectionManager = require('./connectionManager');

class NetworkEngine {
    constructor(port = 5000, partyName = "EchoSync Party") {
        this.port = port;
        this.partyName = partyName;
        
        this.discovery = new DiscoveryModule();
        this.transport = new TransportModule(this.port);
        this.connectionManager = new ConnectionManager();
    }

    // Role: Host
    startHostMode() {
        console.log("======================================");
        console.log(`👑 Starting EchoSync Network Engine (HOST)`);
        console.log("======================================");
        
        // 1. Start Transport Layer (UDP Server)
        this.transport.startServer((msg, rinfo) => {
            const deviceId = `${rinfo.address}:${rinfo.port}`;
            this.connectionManager.addDevice(deviceId, rinfo);
            // Handle sync pings or audio ACKs here
            // console.log(`Received message from ${deviceId}: ${msg}`);
        });

        // 2. Start Discovery Layer (mDNS & BLE)
        this.discovery.startAdvertising(this.port, this.partyName);
        this.discovery.startBLEAdvertising();
    }

    // Role: Client (Speaker Member)
    startClientMode() {
        console.log("======================================");
        console.log(`🔊 Starting EchoSync Network Engine (CLIENT)`);
        console.log("======================================");
        
        // 1. Start scanning for host via mDNS
        this.discovery.scanForHosts((service) => {
            const hostIp = service.addresses[0];
            const hostPort = service.port;
            
            console.log(`🔗 Connecting to Host at ${hostIp}:${hostPort} via UDP...`);
            
            // 2. Connect to Host via Transport Layer
            this.transport.sendToAddress("JOIN_PARTY_REQUEST", hostPort, hostIp);
        });
        
        // 3. Fallback scan for BLE
        this.discovery.scanBLE();
    }
}

// Quick Test if run directly:
if (require.main === module) {
    const args = process.argv.slice(2);
    const engine = new NetworkEngine(5000, "My Awesome Party");

    if (args[0] === 'client') {
        engine.startClientMode();
    } else {
        engine.startHostMode();
    }
}

module.exports = NetworkEngine;
