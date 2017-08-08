// pathfinder/demo/index.ts

const liveServer = require('live-server');

const serverParams = {
    open: false,
    file: "index.html",
    mount: [
        ["/css/bootstrap", "node_modules/bootstrap/dist/css"],
        ["/js/bootstrap", "node_modules/bootstrap/dist/js"],
        ["/js/jquery", "node_modules/jquery/dist"],
        ["/js/pathfinder.js", "pathfinder.js"]
    ]
};

liveServer.start(serverParams);
