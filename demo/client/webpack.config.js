const Handlebars = require('handlebars');
const HandlebarsPlugin = require('handlebars-webpack-plugin');
const fs = require('fs');

module.exports = {
    devtool: 'inline-source-map',
    entry: {
        '3d-demo': "./src/3d-demo.ts",
        'svg-demo': "./src/svg-demo.ts",
        'text-demo': "./src/text-demo.ts",
    },
    module: {
        rules: [
            {
                test: /src\/[a-zA-Z0-9_-]+\.tsx?$/,
                use: 'ts-loader',
                exclude: /node_modules/,
            },
        ]
    },
    resolve: {
        extensions: [".tsx", ".ts", ".html", ".js"],
    },
    output: {
        filename: "[name].js",
        path: __dirname,
    },
    plugins: [
        new HandlebarsPlugin({
            entry: "html/*.hbs",
            output: "./[name]",
            partials: ["html/partials/*.hbs"],
            helpers: {
                octicon: function(iconName) {
                    const svg = fs.readFileSync(`node_modules/octicons/build/svg/${iconName}.svg`);
                    return new Handlebars.SafeString(svg);
                }
            },
        })
    ]
}
