const dgram = require('dgram');

class TransportModule {
    constructor(port = 5000) {
        this.port = port;
        this.socket = dgram.createSocket('udp4');
        this.clients = new Map(); // Store connected client addresses
    }

    // Start the UDP Server (for Host)
    startServer(onMessageReceived) {
        this.socket.on('error', (err) => {
            console.error(`❌ [UDP Transport] Server error:\n${err.stack}`);
            this.socket.close();
        });

        this.socket.on('message', (msg, rinfo) => {
            // Keep track of clients that send messages
            const clientKey = `${rinfo.address}:${rinfo.port}`;
            if (!this.clients.has(clientKey)) {
                console.log(`🔌 [UDP Transport] New device connected: ${clientKey}`);
                this.clients.set(clientKey, rinfo);
            }

            if (onMessageReceived) {
                onMessageReceived(msg, rinfo);
            }
        });

        this.socket.on('listening', () => {
            const address = this.socket.address();
            console.log(`🚀 [UDP Transport] Server listening on ${address.address}:${address.port}`);
        });

        this.socket.bind(this.port);
    }

    // Send audio packet or sync ping to all connected clients (Host -> Clients)
    broadcastToClients(message) {
        const buffer = Buffer.from(message);
        for (const [key, rinfo] of this.clients.entries()) {
            this.socket.send(buffer, rinfo.port, rinfo.address, (err) => {
                if (err) console.error(`❌ [UDP Transport] Error sending to ${key}:`, err);
            });
        }
    }

    // Send data to a specific address (Client -> Host, or Sync Pings)
    sendToAddress(message, port, address) {
        const buffer = Buffer.from(message);
        this.socket.send(buffer, port, address, (err) => {
            if (err) console.error(`❌ [UDP Transport] Error sending to ${address}:${port}:`, err);
        });
    }

    close() {
        this.socket.close();
        console.log(`🛑 [UDP Transport] Socket closed.`);
    }
}

module.exports = TransportModule;
