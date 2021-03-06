const path = require("path");
const HtmlWebpackPlugin = require("html-webpack-plugin");

const dist = path.resolve(__dirname, "pkg/dist");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

module.exports = {
    entry: "./js/index.js",
    output: {
        path: dist,
        filename: "bundle.js"
    },
    devServer: {
        contentBase: dist,
        host: '0.0.0.0'
    },
    module: {
        rules: [
            {
                test: /\.css$/,
                use: ['style-loader', 'css-loader']
            },
            {
                test: /\.(ttf|eot|woff|woff2|svg)$/,
                use: ['file-loader'],
            },
        ]
    },
    plugins: [
        new HtmlWebpackPlugin({
            template: './index.html'
        }),

        new WasmPackPlugin({
            crateDirectory: path.resolve(__dirname, "rust")
        }),
    ]
};
