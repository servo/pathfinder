const {spawn} = require('child_process');

class RustdocPlugin {
    constructor(options) {
        this.options = Object.assign({
            directories: [],
            onBeforeSetup: Function.prototype,
            onBeforeAddPartials: Function.prototype,
            onBeforeCompile: Function.prototype,
            onBeforeRender: Function.prototype,
            onBeforeSave: Function.prototype,
            onDone: Function.prototype
        }, options);
    }

    apply(compiler) {
        compiler.plugin("make", (compilation, done) => {
            let directoriesLeft = this.options.directories.length;
            for (const directory of this.options.directories) {
                console.log("Building documentation for `" + directory + "`...");
                const cargo = spawn("cargo", ["doc"], {cwd: directory});
                cargo.stdout.setEncoding('utf8');
                cargo.stderr.setEncoding('utf8');
                cargo.stdout.on('data', data => console.log(data));
                cargo.stderr.on('data', data => console.log(data));
                cargo.on('close', code => {
                    if (code !== 0) {
                        const message = "Failed to build documentation for `" + directory + "`!";
                        console.error(message);
                        throw new Error(message);
                    }

                    console.log("Built documentation for `" + directory + "`.");
                    directoriesLeft--;
                    if (directoriesLeft === 0)
                        done();
                });
            }
        });
    }
}

module.exports = RustdocPlugin;
