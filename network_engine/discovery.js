const { Bonjour } = require('bonjour-service');
const os = require('os');

class DiscoveryModule {
    constructor() {
        this.bonjour = new Bonjour();
        this.service = null;
    }

    // Host device advertises its presence using mDNS
    startAdvertising(port = 5000, partyName = "EchoSync Party") {
        console.log(`📡 [mDNS] Advertising EchoSync Host on port ${port}...`);
        
        this.service = this.bonjour.publish({
            name: partyName,
            type: 'echosync',
            protocol: 'udp',
            port: port,
            txt: { status: 'ready', role: 'host' }
        });

        this.service.on('up', () => {
            console.log(`✅ [mDNS] Service is up: ${this.service.name}`);
        });
    }

    stopAdvertising() {
        if (this.service) {
            this.service.stop();
            console.log(`🛑 [mDNS] Stopped advertising.`);
        }
    }

    // Client device scans for nearby hosts
    scanForHosts(callback) {
        console.log(`🔍 [mDNS] Scanning for EchoSync hosts...`);
        const browser = this.bonjour.find({ type: 'echosync', protocol: 'udp' });

        browser.on('up', (service) => {
            console.log(`🎉 [mDNS] Found Host: ${service.name} at ${service.addresses[0]}:${service.port}`);
            if (callback) callback(service);
        });
    }

    // Stub for BLE Discovery (Would use @abandonware/noble for actual hardware BLE)
    startBLEAdvertising() {
        console.log("🟦 [BLE] BLE Advertising started (Stub) - Host Mode");
    }

    scanBLE() {
        console.log("🟦 [BLE] BLE Scanning started (Stub) - Client Mode");
    }
}

module.exports = DiscoveryModule;
