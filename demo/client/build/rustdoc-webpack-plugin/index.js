const {spawn} = require('child_process');
const process = require('process');

class RustdocPlugin {
    constructor(options) {
        this.options = Object.assign({directories: [], flags: {}}, options);
    }

    apply(compiler) {
        let rustdocFlags = [];
        for (let key in this.options.flags) {
            if (this.options.flags.hasOwnProperty(key))
                rustdocFlags.push("--" + key + "=" + this.options.flags[key]);
        }
        rustdocFlags = rustdocFlags.join(" ");

        compiler.plugin('after-compile', (compilation, done) => {
            let directoriesLeft = this.options.directories.length;
            for (const directory of this.options.directories) {
                console.log("Building documentation for `" + directory + "`...");
                const cargo = spawn("cargo", ["doc", "--no-deps"], {
                    cwd: directory,
                    env: Object.assign({RUSTDOCFLAGS: rustdocFlags}, process.env),
                });
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
