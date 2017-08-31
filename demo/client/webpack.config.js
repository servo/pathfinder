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
                test: /\.tsx?$/,
                use: 'ts-loader',
                exclude: /node_modules/,
            },
            {
                test: /\.svg?$/,
                use: 'svg-inline-loader',
            },
            {
                test: /html\/[a-zA-Z0-9_-]+\.html$/,
                use: [
                    {
                        loader: 'file-loader',
                        options: {
                            name: "[name].html",
                        },
                    },
                    'extract-loader',
                    {
                        loader: 'html-loader',
                        options: {
                            interpolate: true,
                        },
                    },
                ],
            },
            {
                test: /html\/include\/[a-zA-Z0-9_-]+\.html$/,
                use: [
                    {
                        loader: 'html-loader',
                        options: {
                            interpolate: true,
                        },
                    },
                ],
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
}
