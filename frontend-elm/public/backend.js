function addFunction(app, name, args, callPort, returnPort) {
    let nextCallId = 0;
    let nextReturnId = 0;
    let waitingResponses = new Map();

    function returnWaiting() {
        while (waitingResponses.has(nextReturnId)) {
            app.ports[returnPort].send(waitingResponses.get(nextReturnId));
            waitingResponses.delete(nextReturnId);
            nextReturnId++;
        }
    }

    if (!(callPort in app.ports))
        throw new Error(`Missing call port: ${callPort}`);
    if (!(returnPort in app.ports))
        throw new Error(`Missing return port: ${returnPort}`);

    app.ports[callPort].subscribe(value => {
        const id = nextCallId++;
        invoke(name, args(value))
            .then(value => {
                if (id == nextReturnId) {
                    app.ports[returnPort].send(value);
                    nextReturnId++;
                    returnWaiting();
                } else {
                    waitingResponses.set(id, value);
                }
            });
    });
}

function initBackend(app) {
    addFunction(app, "increment", text => ({ text }), "callIncrement", "returnIncrement");
    addFunction(app, "decrement", text => ({ text }), "callDecrement", "returnDecrement");
}