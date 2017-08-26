module.exports = {
    devtool: 'inline-source-map',
    entry: {
        svg: "./src/svg.ts",
        text: "./src/text.ts",
    },
    module: {
        rules: [
            {
                test: /\.tsx?$/,
                use: 'ts-loader',
                exclude: /node_modules/,
            }
        ]
    },
    resolve: {
        extensions: [".tsx", ".ts", ".js"],
    },
    output: {
        filename: "[name].js",
        path: __dirname,
    },
}
