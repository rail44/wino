import '@fortawesome/fontawesome-free/css/all.css';
import 'bulma/css/bulma.css';
import("../rust/pkg").then(module => {
  module.run();
});
