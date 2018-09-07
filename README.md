jsonrpc-proxy
=================================

# Usage

```
rpc-proxy 0.1
Parity Technologies Ltd <admin@parity.io>
Generic RPC proxy, featuring caching and load balancing.

USAGE:
    rpc-proxy [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --cached-methods-path <cached-methods-path>
            A path to a JSON file containing a list of methods that should be
            cached. See examples for the file schema. [default: -]
        --http-cors <http-cors>
            Specify CORS header for HTTP JSON-RPC API responses.Special options:
            "all", "null", "none". [default: none]
        --http-cors-max-age <http-cors-max-age>
            Configures AccessControlMaxAge header value in milliseconds.Informs
            the client that the preflight request is not required for the
            specified time. Use 0 to disable. [default: 3600000]
        --http-hosts <http-hosts>
            List of allowed Host header values. This option willvalidate the
            Host header sent by the browser, it isadditional security against
            some attack vectors. Specialoptions: "all", "none". [default: none]
        --http-ip <http-ip>
            Configures HTTP server interface. [default: 127.0.0.1]

        --http-max-payload <http-max-payload>
            Maximal HTTP server payload in Megabytes. [default: 5]

        --http-port <http-port>
            Configures HTTP server listening port. [default: 9934]

        --http-rest-api <http-rest-api>
            Enables REST -> RPC converter for HTTP server. Allows you tocall RPC
            methods with `POST /<methodname>/<param1>/<param2>`.The "secure"
            option requires the `Content-Type: application/json`header to be
            sent with the request (even though the payload is ignored)to prevent
            accepting POST requests from any website (via form submission).The
            "unsecure" option does not require any `Content-Type`.Possible
            options: "unsecure", "secure", "disabled". [default: disabled]
        --http-threads <http-threads>
            Configures HTTP server threads. [default: 4]

        --ipc-path <ipc-path>
            Configures IPC server socket path. [default: ./jsonrpc.ipc]

        --ipc-request-separator <ipc-request-separator>
            Configures TCP server request separator (single byte). If "none" the
            parser will try to figure out requests boundaries. [default: none]
        --tcp-ip <tcp-ip>
            Configures TCP server interface. [default: 127.0.0.1]

        --tcp-port <tcp-port>
            Configures TCP server listening port. [default: 9955]

        --tcp-request-separator <tcp-request-separator>
            Configures TCP server request separator (single byte). If "none" the
            parser will try to figure out requests boundaries. Default is new
            line character. [default: 10]
        --upstream-ws <upstream-ws>
            Address of the parent WebSockets RPC server that we should connect
            to. [default: ws://127.0.0.1:9944]
        --websockets-hosts <websockets-hosts>
             List of allowed Host header values. This option will validate the
            Host header sent by the browser, it is additional security against
            some attack vectors. Special options: "all", "none". [default: none]
        --websockets-ip <websockets-ip>
            Configures WebSockets server interface. [default: 127.0.0.1]

        --websockets-max-connections <websockets-max-connections>
            Maximum number of allowed concurrent WebSockets JSON-RPC
            connections. [default: 100]
        --websockets-origins <websockets-origins>
             Specify Origin header values allowed to connect. Special options:
            "all", "none".  [default: none]
        --websockets-port <websockets-port>
            Configures WebSockets server listening port. [default: 9945]
```
